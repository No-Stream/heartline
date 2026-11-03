import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import { expect, test } from "@playwright/test";

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

type Window = { constructionActions: unknown[]; setConstructionState(state: object): void };
const recorded = (page: import("@playwright/test").Page) => page.evaluate(() => (window as unknown as Window).constructionActions);

test("offers the wooden twister in the model chooser @fast", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as Window).setConstructionState({ active: false }));
  const model = page.getByRole("combobox", { name: "Coaster model" });
  await model.selectOption("model:wooden_twister");
  await expect(model.locator("option:checked")).toHaveText("Briar Blitz · Great Coaster Workshop");
  await expect(page.locator("[data-model-description]")).toContainText("wooden");
  expect(await recorded(page)).toContainEqual({ type: "construction-set-model", model: "wooden_twister" });
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
  expect(await recorded(page)).toEqual([{ type: "construction-set-open", open: true }, { type: "construction-set-open", open: false }]);
});

test("construction controls dispatch actions with the keyboard and reflect worker validity", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.getByRole("button", { name: "Place station" }).focus();
  await page.keyboard.press("Enter");
  await page.getByLabel("Station height above ground (metres)").fill("80");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("radio", { name: "Standalone small left turn" }).focus();
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

test("piece steppers and palette show the choice, and refused pieces explain why", async ({ page }) => {
  await page.goto(fixtureUrl);
  const direction = page.getByRole("group", { name: "Next piece turn" });
  await expect(direction).toContainText("Ahead");
  await expect(page.getByRole("button", { name: "Turn right" })).toBeDisabled();
  await page.getByRole("button", { name: "Turn left" }).click();
  await expect(direction).toContainText("Left 90°");
  await expect(page.getByRole("radio", { name: "Standalone small left turn" })).toHaveAttribute("aria-checked", "true");
  await expect(page.locator("[data-preview-name]")).toHaveText("Standalone small left turn");
  const refused = page.getByRole("radio", { name: "Begin gentle climb" });
  await expect(refused).toHaveClass(/piece-tile--refused/);
  await expect(refused).toHaveAccessibleDescription(/The train would collide with terrain\./);
  await refused.focus();
  await expect(page.getByRole("tooltip").filter({ hasText: "collide with terrain" })).toBeVisible();
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  await expect(page.locator("[data-track] li").last()).toContainText("Straight");
});

test("the whole build loop runs from the keyboard with the panel focused", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.getByRole("button", { name: "Undo last piece" }).focus();
  for (const key of ["a", "d", "w", "s", "z", "]", "[", "1", "3", "Enter", "Backspace", "Delete", "c", "t", "Escape"]) await page.keyboard.press(key);
  await expect(page.getByRole("checkbox", { name: /Delete track by clicking it/ })).not.toBeChecked();
  expect(await recorded(page)).toEqual([
    { type: "piece-step", attribute: "turn", step: -1 }, { type: "construction-select", piece: "TurnSmallStandalone" },
    { type: "piece-step", attribute: "turn", step: 1 }, { type: "construction-select", piece: "Straight" },
    { type: "piece-step", attribute: "slope", step: 1 }, { type: "construction-select", piece: "LevelToGentleUp" },
    { type: "piece-step", attribute: "slope", step: -1 }, { type: "construction-select", piece: "Straight" },
    { type: "piece-step", attribute: "bank", step: -1 },
    { type: "piece-step", attribute: "piece", step: 1 }, { type: "construction-select", piece: "TurnSmallStandalone" },
    { type: "piece-step", attribute: "piece", step: -1 }, { type: "construction-select", piece: "Straight" },
    { type: "piece-group", group: "straight" },
    { type: "piece-group", group: "slope" }, { type: "construction-select", piece: "LevelToGentleUp" },
    { type: "construction-undo" },
    { type: "construction-undo" },
    { type: "delete-mode-toggle" }, { type: "construction-close" }, { type: "construction-test" }, { type: "builder-cancel" },
  ]);
  await page.getByRole("radio", { name: "Begin gentle climb" }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByRole("radio", { name: "Standalone small left turn" })).toBeFocused();
  await page.keyboard.press("Enter");
  expect((await recorded(page)).slice(-2)).toEqual([{ type: "construction-select", piece: "TurnSmallStandalone" }, { type: "construction-append" }]);
});

test("testing panel shows an all-clear for a report without notes", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as { setTestingReport(report: object): void }).setTestingReport({ samples: [], notes: [], proximityEvents: [], rating: { excitement: 20, intensity: 30, nausea: 5 } }));
  await expect(page.locator("[data-notes]")).toHaveText("No concerns were noted during this test.");
  await expect(page.locator("[data-rating]")).toHaveText("Excitement: 20.0 · Intensity: 30.0 · Nausea: 5.0");
});

test("testing panel groups proximity events and shows their excitement contribution", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as { setTestingReport(report: object): void }).setTestingReport({
    samples: [], notes: [], rating: { excitement: 35, intensity: 30, nausea: 5 }, proximityEvents: [
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
  await expect(events).toContainText("Added 2.4 excitement from proximity events.");
});
