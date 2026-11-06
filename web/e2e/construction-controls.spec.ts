import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import { expect, test } from "@playwright/test";
import { COASTER_MODELS } from "../src/generated/coaster-models";
import { choosePiece, closeCircuit, openRide, placePiece, placeStation, showBuilderTab, startPark, testRide } from "./player/driver";

let server: PreviewServer;
let outDir: string;
let fixtureUrl: string;
test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "construction-controls-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/construction.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/construction.html`;
});
test.afterAll(async () => {
  if (server) await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  if (outDir) await rm(outDir, { recursive: true });
});

type Window = { constructionActions: unknown[]; setConstructionState(state: object): void; setConstructionUnits(units: "metric" | "imperial"): void };
const recorded = (page: import("@playwright/test").Page) => page.evaluate(() => (window as unknown as Window).constructionActions);

test("busy construction controls remain interactive and accept button input", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ complete: false }));
  const editor = page.locator("[data-editor]");
  const append = page.getByRole("button", { name: "Place piece" });
  await expect(editor).toHaveAttribute("aria-busy", "true");
  await expect(editor).not.toHaveAttribute("inert", "");
  await expect(append).toBeEnabled();
  await append.click();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ complete: true }));
  await page.getByRole("radio", { name: "Small left turn" }).click();
  expect(await recorded(page)).toEqual([
    { type: "construction-append" },
    { type: "construction-select", piece: "TurnSmallStandalone" },
  ]);
});

test("choosing the already-selected piece group does not dispatch another selection", async ({ page }) => {
  await page.goto(fixtureUrl);
  const actionsBefore = await recorded(page);
  await page.getByRole("button", { name: "Undo last piece" }).focus();
  await page.keyboard.press("1");
  const actionsAfter = await recorded(page);
  expect(actionsAfter.filter((action) => (action as { type?: string }).type === "construction-select"))
    .toEqual(actionsBefore.filter((action) => (action as { type?: string }).type === "construction-select"));
});

test("player driver places a station in the live park", async ({ page }) => {
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 50, y: 50, z: 2 }, direction: { x: 52, y: 50, z: 2 } });
  await expect(page.locator("[data-track] li")).toContainText(["Station"]);
});

test("keyboard shortcuts queue in order while live construction validation is busy", async ({ page }) => {
  await page.addInitScript(() => {
    interface ConstructionMessageGate {
      enabled: boolean;
      sawBusy: boolean;
      holding: boolean;
      pending: Array<() => void>;
      release(): void;
    }
    const gate: ConstructionMessageGate = {
      enabled: true,
      sawBusy: false,
      holding: false,
      pending: [],
      release() {
        this.enabled = false;
        this.holding = false;
        this.sawBusy = false;
        for (const deliver of this.pending.splice(0)) deliver();
      },
    };
    Object.assign(window, { constructionMessageGate: gate });
    const workerPrototype = Worker.prototype as unknown as {
      addEventListener(type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | AddEventListenerOptions): void;
    };
    const originalAddEventListener = workerPrototype.addEventListener;
    workerPrototype.addEventListener = function (this: Worker, type, listener, options): void {
      if (type !== "message" || typeof listener !== "function") {
        originalAddEventListener.call(this, type, listener, options);
        return;
      }
      const worker = this;
      const gatedListener: EventListener = (event) => {
        const message = event as MessageEvent<{ type?: string; active?: boolean; complete?: boolean }>;
        const isConstructionState = message.data?.type === "construction-state";
        if (gate.enabled && isConstructionState && message.data.active === true && message.data.complete === false) gate.sawBusy = true;
        if (gate.enabled && isConstructionState && (gate.holding || (gate.sawBusy && message.data.active === true && message.data.complete === true))) {
          gate.holding = true;
          gate.pending.push(() => listener.call(worker, event));
        } else listener.call(worker, event);
      };
      originalAddEventListener.call(this, type, gatedListener, options);
    };
  });
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 50, y: 50, z: 2 }, direction: { x: 52, y: 50, z: 2 } });
  await page.locator("canvas").focus();

  const editor = page.locator("[data-construction] [data-editor]");
  const track = page.locator("[data-track] li");
  const queueNotice = page.locator("[data-construction-queue]");
  await expect(editor).toHaveAttribute("aria-busy", "true");
  for (const key of ["h", "b"]) await page.keyboard.press(key);
  await page.locator("[data-chain-toggle]").click();
  await page.locator("canvas").focus();
  await page.keyboard.press("b");
  await expect(queueNotice).toHaveText("4 actions queued");
  await page.evaluate(() => (window as unknown as { constructionMessageGate: { release(): void } }).constructionMessageGate.release());
  await expect(queueNotice).toHaveText("");

  await expect(track).toHaveCount(3);
  await expect(page.locator("[data-chain-marker]")).toHaveCount(1);
  await expect(page.locator("[data-chain-toggle]")).toHaveAttribute("aria-pressed", "false");
});

test("player driver uses the construction steppers and actions", async ({ page }) => {
  await page.goto(fixtureUrl);
  await choosePiece(page, [{ attribute: "turn", direction: "left" }], "Small left turn");
  await placePiece(page);
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ canClose: true, canTest: true }));
  await closeCircuit(page);
  await testRide(page);
  await openRide(page);
  expect(await recorded(page)).toEqual([
    { type: "piece-step", attribute: "turn", step: -1 },
    { type: "construction-select", piece: "TurnSmallStandalone", exact: true },
    { type: "construction-append" },
    { type: "construction-close" },
    { type: "construction-test" },
    { type: "construction-open-toggle" },
  ]);
});

test("offers generated coaster models in the model chooser @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ active: false }));
  const model = page.getByRole("combobox", { name: "Coaster model" });
  await expect(model.locator("option")).toHaveCount(COASTER_MODELS.length);
  await expect(model.locator("option").evaluateAll((options) => options.map((option) => ({
    value: (option as HTMLOptionElement).value,
    label: option.textContent,
  })))).resolves.toEqual(COASTER_MODELS.map(({ key, name, manufacturer }) => ({
    value: `model:${key}`,
    label: `${name} · ${manufacturer}`,
  })));

  for (const { key, description } of COASTER_MODELS) {
    await model.selectOption(`model:${key}`);
    await expect(page.locator("[data-model-description]")).toHaveText(description);
    expect(await recorded(page)).toContainEqual({ type: "construction-set-model", model: key });
  }
});

test("opens and closes a completed coaster", async ({ page }) => {
  await page.goto(fixtureUrl);
  await expect(page.locator("[data-station-access]")).toContainText("queue at (227, 251)");
  await expect(page.locator("[data-station-access]")).toContainText("exit path at (227, 253)");
  const open = page.getByRole("button", { name: "Open coaster" });
  await expect(open).toBeDisabled();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ canTest: true }));
  await open.click();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ isOpen: true }));
  await page.getByRole("button", { name: "Close coaster" }).click();
  expect(await recorded(page)).toEqual([{ type: "construction-open-toggle" }, { type: "construction-open-toggle" }]);
});

test("construction controls dispatch actions with the keyboard and reflect worker validity", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.getByRole("button", { name: "Place station" }).focus();
  await page.keyboard.press("Enter");
  await page.getByLabel("Station height above ground (metres)").fill("80");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("radio", { name: "Small left turn" }).focus();
  await page.keyboard.press("Space");
  await page.getByRole("button", { name: "Place piece" }).focus();
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Delete Straight, piece 2" }).focus();
  await page.keyboard.press("Enter");
  expect(await recorded(page)).toEqual([
    { type: "station-placement", enabled: true },
    { type: "station-height", height: 80 },
    { type: "construction-select", piece: "TurnSmallStandalone" },
    { type: "construction-append" },
    { type: "construction-delete", id: 9 },
  ]);
  await expect(page.getByRole("button", { name: "Close circuit" })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ canAppend: false, message: "Track collides" }));
  await expect(page.getByRole("button", { name: "Place piece" })).toBeDisabled();
  await expect(page.getByRole("status")).toHaveText("Track collides");
});

test("finish controls explain missing connections and confirm or cancel a preview @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  const finish = page.getByRole("button", { name: "Finish coaster" });
  const reason = page.locator("[data-finish-reason]");
  await expect(finish).toBeDisabled();
  await expect(reason).toHaveText("Build more track to reach a connection.");

  await page.evaluate(() => (window as unknown as Window).setConstructionState({ canFinish: true, finishReason: "" }));
  await expect(finish).toBeEnabled();
  await finish.click();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ finishPreview: true }));
  await expect(page.getByRole("button", { name: "Confirm coaster finish" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel finish preview" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel finish preview" }).click();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ finishPreview: false }));
  await finish.click();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ finishPreview: true }));
  await page.getByRole("button", { name: "Confirm coaster finish" }).click();

  expect(await recorded(page)).toEqual([
    { type: "construction-finish-preview" },
    { type: "construction-finish-cancel" },
    { type: "construction-finish-preview" },
    { type: "construction-finish-confirm" },
  ]);
});

test("station height stepper stays within its range in two-metre steps", async ({ page }) => {
  await page.goto(fixtureUrl);
  const height = page.getByLabel("Station height above ground (metres)");
  await page.getByRole("button", { name: "Raise the station" }).click();
  await page.getByRole("button", { name: "Raise the station" }).click();
  await expect(height).toHaveValue("16");
  await page.getByRole("button", { name: "Lower the station" }).click();
  await expect(height).toHaveValue("14");
  await height.fill("7");
  await height.press("Tab");
  await expect(height).toHaveValue("8");
  await height.fill("500");
  await height.press("Tab");
  await expect(height).toHaveValue("100");
  await page.getByRole("button", { name: "Raise the station" }).click();
  await height.fill("1");
  await height.press("Tab");
  await expect(height).toHaveValue("4");
  expect(await recorded(page)).toEqual([14, 16, 14, 8, 100, 4].map((value) => ({ type: "station-height", height: value })));
});

test("station heights convert when units change and inputs dispatch metres", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as Window).setConstructionUnits("imperial"));
  const height = page.getByLabel("Station height above ground (feet)");
  await expect(height).toHaveValue("39");
  await expect(height).toHaveAttribute("step", "7");
  await page.getByRole("button", { name: "Raise the station" }).click();
  await expect(height).toHaveValue("46");
  await page.getByRole("button", { name: "Lower the station" }).click();
  await expect(height).toHaveValue("39");
  await height.fill("66");
  await height.press("Tab");
  expect(await recorded(page)).toContainEqual({ type: "station-height", height: 20 });
  await page.evaluate(() => (window as unknown as Window).setConstructionUnits("metric"));
  await expect(page.getByLabel("Station height above ground (metres)")).toHaveValue("20");
});

test("block boundaries display and dispatch in the selected units", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => {
    const fixture = window as unknown as Window;
    fixture.setConstructionState({ canTest: true, blockCandidates: new Float64Array([10, 20]), blockReport: {
      sections: [{ boundaryMetres: 10, brakeStartMetres: 8, brakePieceIndices: [], poweredHoldStartMetres: null, poweredHoldPieceIndices: [], editable: true, editableIndex: 0, reason: "", trackKind: "blockBrake" }],
      trainCapacity: 2, capacitySearchComplete: true, theoreticalHourlyRiders: null, throughputStatus: "unavailable", placementStatus: "manual", placementSearchComplete: true, reasons: [], notes: [], suggestions: [],
    } });
    fixture.setConstructionUnits("imperial");
  });
  await showBuilderTab(page, "Operate");
  const input = page.getByLabel("Block 1 boundary in feet");
  await expect(input).toHaveValue("33");
  await input.fill("66");
  await page.getByRole("button", { name: "Move block", exact: true }).click();
  const actions = await recorded(page) as { type: string; boundaryMetres: number }[];
  expect(actions.at(-1)?.type).toBe("construction-move-block");
  expect(actions.at(-1)?.boundaryMetres).toBeCloseTo(66 * 0.3048, 8);
  await page.evaluate(() => (window as unknown as Window).setConstructionUnits("metric"));
  await expect(page.getByLabel("Block 1 boundary in metres")).toHaveValue("10");
});

test("piece steppers and palette show the choice, and refused pieces explain why", async ({ page }) => {
  await page.goto(fixtureUrl);
  const direction = page.getByRole("group", { name: "Next piece turn" });
  await expect(direction).toContainText("Ahead");
  await expect(page.getByRole("button", { name: "Turn right" })).toBeDisabled();
  await page.getByRole("button", { name: "Turn left" }).click();
  await expect(direction).toContainText("Left 90°");
  await expect(page.getByRole("radio", { name: "Small left turn" })).toHaveAttribute("aria-checked", "true");
  await expect(page.locator("[data-preview-name]")).toHaveText("Small left turn");
  const refused = page.getByRole("radio", { name: "Begin gentle climb" });
  await expect(refused).toHaveClass(/piece-tile--refused/);
  await expect(refused).toHaveAccessibleDescription(/The train would collide with terrain\./);
  await refused.focus();
  await expect(page.getByRole("tooltip").filter({ hasText: "collide with terrain" })).toBeVisible();
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  await expect(page.locator("[data-track] li").last()).toContainText("Straight");
});

test("the chain lift toggle asks for the opposite of the worker's chain state and shows that state @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  const chain = page.getByRole("button", { name: "Chain lift", exact: true });
  await expect(chain).toHaveAttribute("aria-pressed", "false");
  await expect(chain).toHaveAccessibleDescription("Turn on to pull trains up the next pieces.");
  await chain.focus();
  await page.keyboard.press("Enter");
  // The fixture stands in for the worker, so nothing changes until a new state arrives.
  await expect(chain).toHaveAttribute("aria-pressed", "false");
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ chain: true }));
  await expect(chain).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("region", { name: "Coaster construction" })).toHaveAttribute("data-chain", "true");
  await expect(chain).toHaveAccessibleDescription(/Hatched pieces cannot carry a chain/);
  await chain.click();
  await page.keyboard.press("h");
  expect(await recorded(page)).toEqual([
    { type: "chain-toggle" },
    { type: "chain-toggle" },
    { type: "chain-toggle" },
  ]);
});

test("the whole build loop runs from the keyboard with the panel focused", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.getByRole("button", { name: "Undo last piece" }).focus();
  for (const key of ["a", "d", "w", "s", "z", "]", "[", "1", "3", "Enter", "Backspace", "Delete", "c", "t", "Escape"]) await page.keyboard.press(key);
  await expect(page.getByRole("checkbox", { name: /Delete track by clicking it/ })).not.toBeChecked();
  expect(await recorded(page)).toEqual([
    { type: "piece-step", attribute: "turn", step: -1 }, { type: "construction-select", piece: "TurnSmallStandalone", exact: true },
    { type: "piece-step", attribute: "turn", step: 1 }, { type: "construction-select", piece: "Straight", exact: true },
    { type: "piece-step", attribute: "slope", step: 1 }, { type: "construction-select", piece: "LevelToGentleUp", exact: true },
    { type: "piece-step", attribute: "slope", step: -1 }, { type: "construction-select", piece: "Straight", exact: true },
    { type: "piece-step", attribute: "bank", step: -1 },
    { type: "piece-step", attribute: "piece", step: 1 }, { type: "construction-select", piece: "TurnSmallStandalone", exact: true },
    { type: "piece-step", attribute: "piece", step: -1 }, { type: "construction-select", piece: "Straight", exact: true },
    { type: "piece-group", group: "straight" },
    { type: "piece-group", group: "slope" }, { type: "construction-select", piece: "LevelToGentleUp" },
    { type: "construction-undo" },
    { type: "construction-undo" },
    { type: "delete-mode-toggle" }, { type: "construction-close" }, { type: "construction-test" }, { type: "builder-cancel" },
  ]);
  await page.getByRole("radio", { name: "Begin gentle climb" }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByRole("radio", { name: "Small left turn" })).toBeFocused();
  await page.keyboard.press("Enter");
  expect((await recorded(page)).slice(-2)).toEqual([{ type: "construction-select", piece: "TurnSmallStandalone", exact: true }, { type: "construction-append" }]);
});

test("the builder keeps every piece choice on the Build tab and settings on Colours and Operate @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  const tab = (name: string) => page.getByRole("tab", { name, exact: true });
  const build = page.getByRole("tabpanel", { name: "Build" });
  await expect(tab("Build")).toHaveAttribute("aria-selected", "true");
  const choices = build.getByRole("region", { name: "Piece choices" });
  for (const control of [
    build.getByRole("group", { name: "Next piece turn" }),
    build.getByRole("button", { name: "Chain lift", exact: true }),
    build.getByRole("button", { name: "Place piece" }),
    choices.getByRole("radiogroup", { name: "Next piece" }),
    build.getByRole("button", { name: "Close circuit" }),
    build.getByRole("button", { name: "Test coaster" }),
    build.getByRole("button", { name: "Open coaster" }),
  ]) await expect(control).toBeVisible();
  // The named track elements share the piece choices region; the fixture model has none to list.
  await expect(choices.locator("[data-elements]")).toBeAttached();
  await expect(page.getByRole("radiogroup", { name: "Track colour" })).toBeHidden();
  await expect(page.getByRole("spinbutton", { name: "Coaster ticket price" })).toBeHidden();

  // Arrow keys move between tabs and stay out of the game's pan shortcuts.
  await tab("Build").focus();
  await page.keyboard.press("ArrowRight");
  await expect(tab("Colours")).toBeFocused();
  await expect(page.getByRole("radiogroup", { name: "Track colour" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Place piece" })).toBeHidden();
  await page.keyboard.press("ArrowRight");
  await expect(tab("Operate")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("spinbutton", { name: "Coaster ticket price" })).toBeVisible();
  await expect(page.locator("[data-block-panel-hint]")).toBeVisible();
  await page.keyboard.press("ArrowRight");
  await expect(tab("Build")).toBeFocused();
  await page.keyboard.press("End");
  await expect(tab("Operate")).toBeFocused();
  expect(await recorded(page)).toEqual([]);

  // Build shortcuts keep working while another tab is shown.
  await page.keyboard.press("b");
  expect(await recorded(page)).toEqual([{ type: "construction-append" }]);

  // A block picked in the world shows Operate, and a new build session starts on Build.
  await showBuilderTab(page, "Colours");
  await page.evaluate(() => {
    const fixture = window as unknown as Window & { selectConstructionBlock(index: number | null): void };
    fixture.setConstructionState({ canTest: true, blockReport: {
      sections: [{ boundaryMetres: 10, brakeStartMetres: 8, brakePieceIndices: [], poweredHoldStartMetres: null, poweredHoldPieceIndices: [], editable: true, editableIndex: 0, reason: "", trackKind: "blockBrake" }],
      trainCapacity: 2, capacitySearchComplete: true, theoreticalHourlyRiders: null, throughputStatus: "unavailable", placementStatus: "manual", placementSearchComplete: true, reasons: [], notes: [], suggestions: [],
    } });
    fixture.selectConstructionBlock(0);
  });
  await expect(tab("Operate")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("region", { name: "Block sections" })).toBeVisible();
  await expect(page.locator("[data-block-panel-hint]")).toBeHidden();
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ active: false }));
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ active: true }));
  await expect(tab("Build")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("button", { name: "Place piece" })).toBeVisible();
});

test("testing panel shows an all-clear for a report without notes", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as { setTestingReport(report: object): void }).setTestingReport({ samples: [], notes: [], proximityEvents: [], rating: { excitement: 2, intensity: 3, nausea: 0.5 }, statistics: {
    ejectorAirtimeSeconds: 0, floaterAirtimeSeconds: 0, airtimeMoments: 0, strongPositiveGSeconds: 0,
    longestStrongPositiveGSeconds: 0, strongLateralGSeconds: 0, maxPositiveVerticalG: 0, maxNegativeVerticalG: 0,
    maxLateralG: 0, maxForwardG: 0, maxBrakingG: 0, maxSpeed: 0, averageSpeed: 0, durationSeconds: 0,
    trackLength: 0, maxHeight: 0, biggestDrop: 0, steepestDropAngleDegrees: 0, inversions: 0,
    invertedSeconds: 0, drops: 0, launches: 0, launchPeakSpeed: 0,
  }, breakdown: { excitement: [], intensity: [], nausea: [] } }));
  await expect(page.locator("[data-notes]")).toHaveText("No concerns were noted during this test.");
  await expect(page.locator("[data-rating]")).toHaveText("Excitement: 2.0 · Intensity: 3.0 · Nausea: 0.5");
});

test("testing panel groups proximity events and shows their excitement contribution", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as { setTestingReport(report: object): void }).setTestingReport({
    samples: [], notes: [], rating: { excitement: 3.5, intensity: 3, nausea: 0.5 }, statistics: {
      ejectorAirtimeSeconds: 0, floaterAirtimeSeconds: 0, airtimeMoments: 0, strongPositiveGSeconds: 0,
      longestStrongPositiveGSeconds: 0, strongLateralGSeconds: 0, maxPositiveVerticalG: 0, maxNegativeVerticalG: 0,
      maxLateralG: 0, maxForwardG: 0, maxBrakingG: 0, maxSpeed: 0, averageSpeed: 0, durationSeconds: 0,
      trackLength: 0, maxHeight: 0, biggestDrop: 0, steepestDropAngleDegrees: 0, inversions: 0,
      invertedSeconds: 0, drops: 0, launches: 0, launchPeakSpeed: 0,
    }, breakdown: { excitement: [{ term: "proximity", value: 0.2, measure: 1.7, unit: "", location: undefined }], intensity: [], nausea: [] }, proximityEvents: [
      { kind: "nearMiss", pieceIndex: 13, distance: 14, x: 2, y: 3, z: 1, strength: 0.8 },
      { kind: "nearMiss", pieceIndex: 8, distance: 9, x: 3, y: 3, z: 1, strength: 0.5 },
      { kind: "dueling", pieceIndex: 7, distance: 8, x: 4, y: 3, z: 1, strength: 0.4 },
    ],
  }));
  const events = page.locator("[data-proximity-events]");
  await expect(events).toContainText("Proximity events (3)");
  await expect(events).toContainText("Near-miss with the coaster's supports (2)");
  await expect(events).toContainText("Dueling trains (1)");
  await expect(events).toContainText("piece 14, very close");
  await expect(events).toContainText("Added 0.2 excitement from proximity events.");
  await expect(page.locator("[data-statistics]")).toBeVisible();
  await expect(page.locator("[data-statistics] h4")).toHaveText([
    "Scores (about 0–10)",
    "Airtime",
    "Forces",
    "Speed and layout",
  ]);
  await expect(page.locator("[data-statistics-airtime] > div")).toHaveText([
    "Ejector airtime: 0.0 s",
    "Floater airtime: 0.0 s",
    "Airtime moments: 0",
  ]);
});

test("testing panel ranks score findings, expands their numbers and inspects a located penalty @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as { setTestingReport(report: object): void }).setTestingReport({
    samples: [], notes: [], proximityEvents: [],
    rating: { excitement: 4.2, intensity: 3.1, nausea: 0.8 },
    statistics: {
      ejectorAirtimeSeconds: 0, floaterAirtimeSeconds: 0, airtimeMoments: 0, strongPositiveGSeconds: 0,
      longestStrongPositiveGSeconds: 0, strongLateralGSeconds: 0, maxPositiveVerticalG: 0, maxNegativeVerticalG: 0,
      maxLateralG: 0, maxForwardG: 0, maxBrakingG: 0, maxSpeed: 0, averageSpeed: 0, durationSeconds: 0,
      trackLength: 0, maxHeight: 0, biggestDrop: 0, steepestDropAngleDegrees: 0, inversions: 0,
      invertedSeconds: 0, drops: 0, launches: 0, launchPeakSpeed: 0,
    },
    breakdown: {
      excitement: [
        { term: "airtime", value: 0.5, measure: 1.2, unit: "s", location: undefined },
        { term: "envelope", value: -1.2, measure: 0.5, unit: "g-s", location: { pieceIndex: 6, pieceName: "Loop", distance: 200, seconds: 8 }, forceDetail: { axis: "vertical", measuredValue: 5, limit: 4, unit: "g", durationSeconds: 2.4, carIndex: 0, location: { pieceIndex: 6, pieceName: "Loop", distance: 200, seconds: 8 } } },
      ],
      intensity: [],
      nausea: [],
    },
  }));

  await expect(page.locator("[data-score-averaging]")).toHaveText("Effects are averaged over all cars. Force numbers come from the car that felt them most; car 1 is the front car.");
  const findings = page.locator("[data-score-findings] [data-score-finding]");
  await expect(findings.locator(".testing-finding__text")).toHaveText([
    "Past the ASTM comfort envelope at Loop: vertical force 5.0 g for 2.4 s (limit 4.0 g)",
    "Airtime: 1.2 s, weighted",
  ]);
  await expect(findings.locator(".testing-finding__effects")).toHaveText(["Excitement −1.2", "Excitement +0.5"]);
  await expect(findings.first().locator(".testing-effect")).toHaveAttribute("data-tone", "bad");
  await expect(findings.last().locator(".testing-effect")).toHaveAttribute("data-tone", "good");
  await expect(page.locator("[data-score-more]")).not.toHaveAttribute("open");
  const envelopeDetails = findings.first().locator(".testing-finding__details");
  await expect(envelopeDetails).toBeHidden();
  await page.locator('[data-score-location="envelope"]').click();
  await expect(page.locator('[data-score-location="envelope"]')).toHaveAttribute("aria-pressed", "true");
  await expect(envelopeDetails).toBeHidden();
  await expect(page.locator("[data-inspection]")).toHaveText("Selected score location: Loop · 656 ft · 8.0 s");
  await findings.first().locator(".testing-finding__text strong").click();
  await expect(envelopeDetails).toBeVisible();
  await expect(envelopeDetails.locator("li").first()).toHaveText("Vertical force 5.0 g for 2.4 s (limit 4.0 g) (car 1) at Loop, 656 ft.");
  expect(await page.evaluate(() => (window as unknown as { selectedScoreLocation(): object }).selectedScoreLocation()))
    .toEqual({ pieceIndex: 6, pieceName: "Loop", distance: 200, seconds: 8 });
});
