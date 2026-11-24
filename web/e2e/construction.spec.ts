import { expect, test } from "@playwright/test";
import { canvasSize, clickCanvas, clickTile, moveOverCanvas, selectUnits } from "./game-page";
import { openCoasterBuilder } from "./coaster-ui";
test("station placement targets ground directly beneath a coaster support", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1.5, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").fill("4");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  await clickTile(page, { x: 50, y: 50, z: 2 });
  await clickCanvas(page, await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 52.5, z: 2 })));
  await page.getByRole("button", { name: "Place piece" }).click();
  await expect(page.locator("[data-track] li")).toHaveCount(2);

  await page.getByRole("button", { name: "Place station" }).click();
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const supportGroundPoint = await page.evaluate(() => {
    const canvas = document.querySelector("canvas")!;
    const center = window.gameTest.project({ x: 52, y: 52, z: 2 });
    for (let y = Math.max(0, center.y - 180); y < Math.min(canvas.height, center.y + 60); y += 1) {
      for (let x = Math.max(0, center.x - 180); x < Math.min(canvas.width, center.x + 180); x += 1) {
        const overlay = window.gameTest.pick(x, y);
        if (overlay?.kind !== "support") continue;
        const ground = window.gameTest.pickSurface(x, y);
        if (ground?.kind === "terrain" && ground.x === overlay.x && ground.y === overlay.y) return { x, y, tile: ground };
      }
    }
    return null;
  });
  expect(supportGroundPoint, "a support should cover a pickable terrain tile").not.toBeNull();

  await moveOverCanvas(page, supportGroundPoint!);
  await expect(page.locator("canvas")).toHaveAttribute("data-station-preview", new RegExp(`${supportGroundPoint!.tile.x},${supportGroundPoint!.tile.y},`));
  await clickCanvas(page, supportGroundPoint!);
  await expect(page.locator("canvas")).toHaveAttribute("data-station-preview", new RegExp(`${supportGroundPoint!.tile.x},${supportGroundPoint!.tile.y},`));
  await clickCanvas(page, supportGroundPoint!);
  await expect(page.locator("[data-track] li")).toHaveCount(1);
});

test("closes a compact circuit with short straights and full-tile offsets @fast", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await selectUnits(page, "metric");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track] li");
  const closeCircuit = page.getByRole("button", { name: "Close circuit" });
  const pieces = [
    "HalfTileStraight",
    "FullOffsetLeft",
    "TurnSmallStandalone",
    "TurnSmallStandalone",
    "Straight",
    "HalfTileStraight",
    "FullOffsetLeft",
    "TurnSmallStandalone",
    "TurnSmallStandalone",
  ];

  await expect(track).toHaveCount(1);
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  await expect(closeCircuit).toBeDisabled();
  for (const [index, piece] of pieces.entries()) {
    const choice = choices.locator(`[data-piece="${piece}"]`);
    await expect(choice).toBeVisible();
    await choice.click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
    await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
    if (index < pieces.length - 1) await expect(closeCircuit).toBeDisabled();
  }

  await expect(track.last()).toContainText("Small left turn");
  await expect(closeCircuit).toBeEnabled();
  await closeCircuit.click();
  await expect(page.getByText("Circuit complete", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
});

test("previews and confirms a lift-powered circuit finish, then completes a ride test @fast", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await selectUnits(page, "metric");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track] li");
  const finishCoaster = page.getByRole("button", { name: "Finish coaster" });
  const pieces = [
    "LevelToGentleUp",
    "GentleUp",
    "GentleUp",
    "GentleUp",
    "GentleUp",
    "GentleUp",
    "GentleUpToLevel",
    "Straight",
    "OneTileStraight",
    "OneTileStraight",
    "OffsetLeft",
    "TurnLargeEntry",
    "TurnLargeExit",
    "LevelToGentleDown",
    "GentleDown",
    "GentleDown",
    "GentleDown",
    "GentleDown",
    "GentleDown",
    "GentleDownToLevel",
    "Loop",
    "OffsetRight",
    "TurnLargeEntry",
    "TurnLargeExit",
    "OneTileStraight",
  ];

  await expect(track).toHaveCount(1);
  await expect(finishCoaster).toBeDisabled();
  await expect(page.locator("[data-finish-reason]")).toHaveText(/\S/);
  for (const [index, piece] of pieces.entries()) {
    if (index === 0 || index === 8) await page.keyboard.press("h");
    const choice = choices.locator(`[data-piece="${piece}"]`);
    await expect(choice).toBeVisible();
    await choice.click();
    if (piece === "Loop") {
      await page.locator('[data-speed-match-variant="Loop"]').click();
    }
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    const refusal = await choice.locator(".piece-tile__tip span").textContent();
    await expect(page.getByRole("button", { name: "Place piece" }), `Cannot place ${piece} at step ${index + 1}: ${refusal}`).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
  }

  await expect(finishCoaster).toBeEnabled();
  await finishCoaster.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-finish-preview", "true");
  await expect(page.getByRole("button", { name: "Confirm coaster finish" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel finish preview" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel finish preview" }).click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-finish-preview", "false");

  await finishCoaster.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-finish-preview", "true");
  const finishOption = page.locator("[data-finish-option]");
  await expect(finishOption).toHaveText(/^Option 1 of (\d+): \d+ pieces \(.+\)\. Arrives at \d+ km\/h/);
  const optionCount = Number(/of (\d+)/.exec(await finishOption.textContent() ?? "")?.[1]);
  const nextOption = page.getByRole("button", { name: "Next option" });
  await expect(nextOption).toBeVisible({ visible: optionCount > 1 });
  if (optionCount > 1) {
    await nextOption.click();
    await expect(finishOption).toHaveText(new RegExp(`^Option 2 of ${optionCount}: `));
    await page.getByRole("button", { name: "Previous option" }).click();
    await expect(finishOption).toHaveText(new RegExp(`^Option 1 of ${optionCount}: `));
  }
  await page.getByRole("button", { name: "Confirm coaster finish" }).click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-finish-preview", "false");
  await expect(page.getByText("Circuit complete", { exact: true })).toBeVisible();
  await expect.poll(() => track.count()).toBeGreaterThan(pieces.length + 1);
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await page.getByRole("button", { name: "Test coaster" }).click();
  await page.evaluate(() => window.gameTest.advanceTicks(10000));
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status")).toHaveText("Train completed a lap.");
  const testingPanel = page.locator("[data-testing]");
  const panelHeight = await testingPanel.evaluate((element) => Math.ceil(element.getBoundingClientRect().height));
  await page.setViewportSize({ width: 1280, height: Math.max(720, panelHeight + 100) });
  await expect(testingPanel).toHaveScreenshot("coaster-testing-panel-lift-circuit.png", { animations: "disabled" });
});

test("places a 60-degree climb and complete banked turn with lift markers @fast", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:hybrid");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const elements = page.locator("[data-elements]");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const chain = page.getByRole("button", { name: "Chain lift", exact: true });
  const chainMarkers = page.locator("[data-track] [data-chain-marker]");
  await expect(track).toHaveCount(1);
  await expect(choices.locator('[data-piece="Straight"]')).toBeVisible();
  for (const [pieceId, name] of [
    ["VerticalUpEntry", "Start of steep upward curve"],
    ["VerticalDownEntry", "Start of steep downward curve"],
  ] as const) {
    const choice = choices.locator(`[data-piece="${pieceId}"]`);
    await expect(choice).toHaveAttribute("data-piece", pieceId);
    await expect(choice).toHaveAccessibleName(name);
  }

  await chain.click();
  await expect(chain).toHaveAttribute("aria-pressed", "true");
  const climbToSixty = elements.locator('[data-element="element:climb-60"]');
  await expect(climbToSixty).toHaveText("Climb to 60°");
  await climbToSixty.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", "element:climb-60");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();

  await expect.poll(() => track.count()).toBeGreaterThan(1);
  const climbTrackCount = await track.count();
  expect(climbTrackCount).toBeGreaterThan(1);
  await expect(elements.locator('[data-element="element:climb-60"]')).toHaveCount(0);
  await expect(chainMarkers).toHaveCount(climbTrackCount - 1);

  await chain.click();
  await expect(chain).toHaveAttribute("aria-pressed", "false");
  const levelOut = elements.locator('[data-element="element:level"]');
  await expect(levelOut).toHaveText("Level out");
  await levelOut.click();
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(elements.locator('[data-element="element:level"]')).toHaveCount(0);
  await expect(chainMarkers).toHaveCount(climbTrackCount - 1);

  const bankedTurn = elements.locator('[data-element="element:small-left-banked-turn"]');
  await expect(bankedTurn).toHaveText("Small left banked turn");
  await bankedTurn.click();
  const trackCountBeforeTurn = await track.count();
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track).toHaveCount(trackCountBeforeTurn + 3);
  await expect(chainMarkers).toHaveCount(climbTrackCount - 1);
  const trackNames = page.locator("[data-track] .track-chip__name");
  await expect(trackNames.nth(trackCountBeforeTurn)).toHaveText("Bank left");
  await expect(trackNames.nth(trackCountBeforeTurn + 1)).toHaveText("Small left-banked turn");
  await expect(trackNames.nth(trackCountBeforeTurn + 2)).toHaveText("Level from left bank");
  for (const index of [trackCountBeforeTurn, trackCountBeforeTurn + 1, trackCountBeforeTurn + 2]) {
    await expect(track.nth(index).locator("[data-chain-marker]")).toHaveCount(0);
  }
});

test("offers direct level-to-60-degree lift and drop transitions on both eligible models @fast", async ({ page }) => {
  for (const model of ["hybrid", "velocity_viper"] as const) {
    await page.goto("/?test=1&park=new&agents=1");
    await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
    await openCoasterBuilder(page);
    await selectUnits(page, "metric");
    await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
    await page.locator(".builder select[data-model]").selectOption(`model:${model}`);
    await page.getByLabel("Station height above ground (metres)").fill("72");
    await page.getByLabel("Station height above ground (metres)").press("Tab");
    await page.getByRole("button", { name: "Place station" }).click();
    for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
      const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
      await moveOverCanvas(page, position);
      await clickCanvas(page, position);
    }

    const track = page.locator("[data-track] li");
    const choices = page.getByRole("radiogroup", { name: "Next piece" });
    const chain = page.getByRole("button", { name: "Chain lift", exact: true });
    const chainMarkers = page.locator("[data-track] [data-chain-marker]");
    let trackCount = 1;
    const placePiece = async (pieceId: string, label: string): Promise<void> => {
      const choice = choices.locator(`[data-piece="${pieceId}"]`);
      await expect(choice).toBeVisible();
      await expect(choice).toHaveAccessibleName(label);
      await choice.click();
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", pieceId);
      const placePieceButton = page.getByRole("button", { name: "Place piece" });
      await expect(placePieceButton).toBeEnabled();
      await placePieceButton.scrollIntoViewIfNeeded();
      await placePieceButton.click();
      trackCount += 1;
      await expect(track).toHaveCount(trackCount);
    };

    await expect(track).toHaveCount(trackCount);
    await chain.click();
    await expect(chain).toHaveAttribute("aria-pressed", "true");
    await placePiece("LevelToSixtyUp", "Begin 60-degree climb");
    await expect(chainMarkers).toHaveCount(1);

    await chain.click();
    await expect(chain).toHaveAttribute("aria-pressed", "false");
    await placePiece("SixtyUpToLevel", "Level out from 60-degree climb");
    await placePiece("LevelToSixtyDown", "Begin 60-degree descent");
    await placePiece("SixtyDownToLevel", "Level out from 60-degree descent");
    await expect(chainMarkers).toHaveCount(1);
  }
});

test("offers and places a twisted drop directly from a banked track @fast", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:hybrid");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  await expect(track).toHaveCount(1);
  const bankLeft = choices.locator('[data-piece="BankLeft"]');
  await expect(bankLeft).toBeVisible();
  await bankLeft.click();
  await placePiece.click();
  await expect(track).toHaveCount(2);

  for (const pieceId of ["BankedDropLeft", "TwistedDropLeft", "BarrelDropLeft"]) {
    await expect(choices.locator(`[data-piece="${pieceId}"]`)).toBeVisible();
  }
  const twistedDrop = choices.locator('[data-piece="TwistedDropLeft"]');
  await expect(twistedDrop).toHaveAccessibleName("Left twisted drop");
  await twistedDrop.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", "TwistedDropLeft");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track).toHaveCount(3);
  await expect(track.last().locator(".track-chip__name")).toHaveText("Left twisted drop");
});

test("sizes an airtime hill for the entry speed, steps it, and repeats it into a camelback run @fast", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:hybrid");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const selected = page.locator("[data-construction]");
  for (const [index, pieceId] of ["LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel"].entries()) {
    await choices.locator(`[data-piece="${pieceId}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", pieceId);
    await placePiece.click();
    await expect(track).toHaveCount(index + 2);
  }

  const hill = page.locator("[data-hill]");
  const readout = hill.locator("[data-hill-readout]");
  await expect(hill).not.toHaveAttribute("open");
  await hill.getByText("Airtime hill").click();
  await expect(selected).toHaveAttribute("data-selected", "EjectorHill8x64");
  await expect(hill.getByRole("group", { name: "Hill feel" })).toContainText("Ejector");
  await expect(hill.locator("[data-hill-suggestion]")).toHaveText("Sized for the predicted entry speed.");
  await expect(readout).toContainText("Entry speed");
  await expect(readout).toContainText("in range");
  await expect(readout).toContainText(/(good|great) airtime/);
  await expect(page.locator("[data-steppers]")).toBeHidden();
  await page.mouse.move(0, 0);
  await expect(hill).toHaveScreenshot("airtime-hill-inspector.png", { animations: "disabled" });

  const speedMatchedReadout = await readout.textContent();
  await hill.getByRole("button", { name: "Taller hill" }).click();
  await expect(selected).toHaveAttribute("data-selected", "EjectorHill12x64");
  await expect(readout).not.toHaveText(speedMatchedReadout!);
  await expect(readout).toContainText("too fast");
  await expect(hill.locator("[data-hill-suggestion]")).toHaveText("Suggested for this speed: 8 × 64 m.");
  await hill.getByRole("button", { name: "Use suggested size" }).click();
  await expect(selected).toHaveAttribute("data-selected", "EjectorHill8x64");

  await placePiece.click();
  await expect(track).toHaveCount(7);
  await expect(selected).toHaveAttribute("data-selected", "EjectorHill8x64");
  await expect(hill).toHaveAttribute("open");
  await placePiece.press("Enter");
  await expect(track).toHaveCount(8);
  await expect(track.nth(6).locator(".track-chip__name")).toHaveText("Ejector hill 8 × 64 m");
  await expect(track.nth(7).locator(".track-chip__name")).toHaveText("Ejector hill 8 × 64 m");
});

test("shows and places a shaped floater hill and fully banked turn on a hyper coaster @fast", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:hyper");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const selected = page.locator("[data-construction]");
  for (const [index, pieceId] of ["LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel"].entries()) {
    await choices.locator(`[data-piece="${pieceId}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", pieceId);
    await placePiece.click();
    await expect(track).toHaveCount(index + 2);
  }

  const hill = page.locator("[data-shaped-inspector]");
  const readout = hill.locator("[data-shape-readout]");
  await expect(hill).not.toHaveAttribute("open");
  await choices.locator('[data-piece="shape:hill:floater:default:auto"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:hill:floater:-0.5:auto");
  await expect(hill.getByRole("group", { name: "Shaped element feel" })).toContainText("Floater");
  await expect(hill.locator('[data-shape-value="strength"]')).toHaveText("−0.5 g");
  await expect(readout).toContainText("Design speed");
  await expect(readout).toContainText("Airtime target: −0.5 g");
  await expect(readout).toContainText("Resolved size:");
  await expect(page.locator("[data-steppers]")).toBeHidden();
  await page.mouse.move(0, 0);
  await expect(hill).toHaveScreenshot("shaped-hill-inspector.png", { animations: "disabled" });

  await hill.getByRole("button", { name: "Increase hill size" }).click();
  await expect(selected).toHaveAttribute("data-selected", /shape:hill:floater:-0\.5:fixed-/);
  await expect(hill.locator('[data-shape-value="size"]')).not.toHaveText("Auto");
  await expect(readout).toContainText("Resolved size:");
  await hill.getByRole("button", { name: "Use automatic hill size" }).click();
  await expect(selected).toHaveAttribute("data-selected", "shape:hill:floater:-0.5:auto");

  await placePiece.click();
  await expect(track).toHaveCount(7);
  await expect(track.nth(6).locator(".track-chip__name")).toHaveText("Shaped airtime hill");
  await expect(track.nth(6).locator("svg.piece-icon")).toBeVisible();

  await choices.locator('[data-piece="shape:turn:left:fullyBanked:auto"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:turn:left:fullyBanked:auto");
  await expect(hill.getByRole("group", { name: "Shaped element feel" })).toContainText("Fully banked");
  await expect(readout).toContainText("Design speed");
  await expect(readout).toContainText("Resolved size:");
  await expect(readout).toContainText("Lateral force at design speed:");

  await placePiece.click();
  await expect(track).toHaveCount(8);
  await expect(track.nth(7).locator(".track-chip__name")).toHaveText("Shaped banked turn left");
  await expect(track.nth(7).locator("svg.piece-icon")).toBeVisible();

  await choices.locator('[data-piece="shape:hill:ejector:default:auto"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:hill:ejector:-1.2:auto");
  await expect(readout.locator(".builder__shape-feel-note")).toHaveText("Within the comfort envelope, stronger than a typical Horizon Hawk.");
  await expect(placePiece).toBeEnabled();
});

for (const modelCase of [
  {
    key: "velocity_viper",
    label: "Velocity Viper",
    stationHeight: "56",
    dropPieces: ["LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel"],
    hillChoice: "shape:hill:floater:default:auto",
    expectedHillId: "shape:hill:floater:-0.5:auto",
  },
  {
    key: "hybrid",
    label: "Timber Tumbler",
    stationHeight: "56",
    dropPieces: ["LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel"],
    hillChoice: "shape:hill:ejector:default:auto",
    expectedHillId: "shape:hill:ejector:-1.05:auto",
  },
  {
    key: "wooden_twister",
    label: "Briar Blitz",
    stationHeight: "48",
    dropPieces: ["LevelToGentleDown", "GentleDown", "GentleDown", "GentleDownToLevel"],
    hillChoice: "shape:hill:floater:default:auto",
    expectedHillId: "shape:hill:floater:-0.5:auto",
  },
] as const) {
  test(`places shaped hill and turn on ${modelCase.label} @fast`, async ({ page }) => {
    await page.goto("/?test=1&park=new");
    await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
    await openCoasterBuilder(page);
    await selectUnits(page, "metric");
    await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
    await page.locator(".builder select[data-model]").selectOption(`model:${modelCase.key}`);
    await page.getByLabel("Station height above ground (metres)").fill(modelCase.stationHeight);
    await page.getByLabel("Station height above ground (metres)").press("Tab");
    await page.getByRole("button", { name: "Place station" }).click();
    for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
      const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
      await moveOverCanvas(page, position);
      await clickCanvas(page, position);
    }

    const track = page.locator("[data-track] li");
    const choices = page.getByRole("radiogroup", { name: "Next piece" });
    const placePiece = page.getByRole("button", { name: "Place piece" });
    const selected = page.locator("[data-construction]");
    for (const [index, pieceId] of modelCase.dropPieces.entries()) {
      await choices.locator(`[data-piece="${pieceId}"]`).click();
      await expect(selected).toHaveAttribute("data-selected", pieceId);
      await placePiece.click();
      await expect(track).toHaveCount(index + 2);
    }

    const baseTrackCount = modelCase.dropPieces.length + 1;
    const readout = page.locator("[data-shape-readout]");
    await choices.locator(`[data-piece="${modelCase.hillChoice}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", modelCase.expectedHillId);
    await expect(readout).toContainText("Design speed:");
    await expect(readout.locator(".builder__shape-feel-note")).toHaveCount(0);
    await expect(placePiece).toBeEnabled();
    await placePiece.click();
    await expect(track).toHaveCount(baseTrackCount + 1);
    await expect(track.nth(baseTrackCount).locator(".track-chip__name")).toHaveText("Shaped airtime hill");

    await choices.locator('[data-piece="shape:turn:left:fullyBanked:auto"]').click();
    await expect(selected).toHaveAttribute("data-selected", "shape:turn:left:fullyBanked:auto");
    await expect(readout).toContainText("Lateral force at design speed:");
    await expect(placePiece).toBeEnabled();
    await placePiece.click();
    await expect(track).toHaveCount(baseTrackCount + 2);
    await expect(track.nth(baseTrackCount + 1).locator(".track-chip__name")).toHaveText("Shaped banked turn left");
  });
}

test("places shaped vertical loop, zero-g roll, and Raven turn on Velocity Viper @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));

  // Velocity Viper supports all three shaped inversions.
  await page.locator(".builder select[data-model]").selectOption("model:velocity_viper");
  await page.getByLabel("Station height above ground (metres)").fill("60");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const selected = page.locator("[data-construction]");
  const readout = page.locator("[data-shape-readout]");

  for (const [index, pieceId] of ["LevelToGentleDown", "GentleToSteepDown", "SteepDown", "SteepToGentleDown", "GentleDownToLevel", "Straight"].entries()) {
    await choices.locator(`[data-piece="${pieceId}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", pieceId);
    await placePiece.click();
    await expect(track).toHaveCount(index + 2);
  }

  // Vertical loop with peak-g and top-feel stepping.
  await choices.locator('[data-piece="shape:loop:left:default:neutral"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:4:neutral");
  await expect(readout).toContainText("Peak target: 4 g");
  await page.locator('[data-shape-stepper="peakG-1"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:4.2:neutral");
  await page.locator('[data-shape-stepper="topFeel--1"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:4.2:floaty");
  await expect(readout).toContainText("(Floaty)");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(7).locator(".track-chip__name")).toHaveText("Shaped vertical loop left");

  // Zero-g roll right after the loop.
  await choices.locator('[data-piece="shape:zeroGRoll:right:default"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:zeroGRoll:right:3.5");
  await expect(readout).toContainText("crest:");
  await expect(readout).toContainText("roll:");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(8).locator(".track-chip__name")).toHaveText("Shaped zero-g roll right");

  // Raven turn reversing direction.
  await choices.locator('[data-piece="shape:ravenTurn:left:default:neutral"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:ravenTurn:left:4:neutral");
  await expect(readout).toContainText("roll:");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(9).locator(".track-chip__name")).toHaveText("Shaped Raven turn left");
});

test("places shaped vertical loop, zero-g roll, and Raven turn on Timber Tumbler @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:hybrid");
  await page.getByLabel("Station height above ground (metres)").fill("60");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const selected = page.locator("[data-construction]");
  const readout = page.locator("[data-shape-readout]");
  for (const pieceId of ["shape:loop:left:default:neutral", "shape:zeroGRoll:right:default", "shape:ravenTurn:left:default:neutral"]) {
    await expect(choices.locator(`[data-piece="${pieceId}"]`)).toHaveCount(1);
  }

  for (const [index, pieceId] of ["LevelToGentleDown", "GentleToSteepDown", "SteepDown", "SteepToGentleDown", "GentleDownToLevel", "Straight"].entries()) {
    await choices.locator(`[data-piece="${pieceId}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", pieceId);
    await placePiece.click();
    await expect(track).toHaveCount(index + 2);
  }

  await choices.locator('[data-piece="shape:loop:left:default:neutral"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:4:neutral");
  await expect(readout).toContainText("Peak target: 4 g");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(7).locator(".track-chip__name")).toHaveText("Shaped vertical loop left");

  await choices.locator('[data-piece="shape:zeroGRoll:right:default"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:zeroGRoll:right:3.5");
  await expect(readout).toContainText("crest:");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(8).locator(".track-chip__name")).toHaveText("Shaped zero-g roll right");

  await choices.locator('[data-piece="shape:ravenTurn:left:default:neutral"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:ravenTurn:left:4:neutral");
  await expect(readout).toContainText("roll:");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(9).locator(".track-chip__name")).toHaveText("Shaped Raven turn left");
});

test("places shaped steel-structure vertical loop on Briar Blitz with amber feel note @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const selected = page.locator("[data-construction]");
  const readout = page.locator("[data-shape-readout]");

  // Briar Blitz offers only steel-structure vertical loops (not zero-g rolls or Raven turns),
  // and pushing peak force above its typical 4.0 g target shows the amber feel note.
  await page.locator(".builder select[data-model]").selectOption("model:wooden_twister");
  await page.getByLabel("Station height above ground (metres)").fill("52");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }
  await expect(choices.locator('[data-piece="shape:loop:left:default:neutral"]')).toHaveCount(1);
  await expect(choices.locator('[data-piece="shape:zeroGRoll:left:default"]')).toHaveCount(0);
  await expect(choices.locator('[data-piece="shape:ravenTurn:left:default:neutral"]')).toHaveCount(0);

  for (const [index, pieceId] of ["LevelToGentleDown", "GentleToSteepDown", "SteepToGentleDown", "GentleDown", "GentleDownToLevel", "Straight"].entries()) {
    await choices.locator(`[data-piece="${pieceId}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", pieceId);
    await placePiece.click();
    await expect(track).toHaveCount(index + 2);
  }
  await choices.locator('[data-piece="shape:loop:left:default:neutral"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:3.5:neutral");
  await expect(readout).toContainText("steel structure");
  await expect(readout.locator(".builder__shape-feel-note")).toHaveCount(0);
  // Step up to 4.2 g (exceeds Briar Blitz's 4.0 g design target inside ASTM envelope).
  await page.locator('[data-shape-stepper="peakG-1"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:3.8:neutral");
  await page.locator('[data-shape-stepper="peakG-1"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:4:neutral");
  await page.locator('[data-shape-stepper="peakG-1"]').click();
  await expect(selected).toHaveAttribute("data-selected", "shape:loop:left:4.2:neutral");
  await expect(readout.locator(".builder__shape-feel-note")).toContainText("stronger than a typical Briar Blitz");
  await expect(placePiece).toBeEnabled();
  await placePiece.click();
  await expect(track.nth(7).locator(".track-chip__name")).toHaveText("Shaped steel-structure loop left");
});

test("turns after a drop default to the bank for the predicted speed, and the bank stepper overrides it @fast", async ({ page }) => {
  await page.goto("/?test=1&park=new");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openCoasterBuilder(page);
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:hybrid");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const track = page.locator("[data-track] li");
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const selected = page.locator("[data-construction]");
  const bank = page.locator('[data-stepper-value="bank"]');
  const sizes = page.getByRole("group", { name: "Element size" });
  const forces = page.locator("[data-candidate-force-preview]");

  await page.getByRole("button", { name: "Turn left" }).click();
  await expect(selected).toHaveAttribute("data-selected", "TurnSmallStandalone");
  await expect(bank).toHaveText("Level");
  await page.getByRole("button", { name: "Turn right" }).click();
  await expect(selected).toHaveAttribute("data-selected", "Straight");

  for (const [index, pieceId] of ["LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel"].entries()) {
    await choices.locator(`[data-piece="${pieceId}"]`).click();
    await expect(selected).toHaveAttribute("data-selected", pieceId);
    await placePiece.click();
    await expect(track).toHaveCount(index + 2);
  }
  await choices.locator('[data-piece="Straight"]').click();
  await expect(selected).toHaveAttribute("data-selected", "Straight");

  await page.getByRole("button", { name: "Turn left" }).click();
  await expect(selected).toHaveAttribute("data-selected", /^SpeedTurnLarge(16|20|24|28|32)Left$/);
  await expect(bank).toHaveText("For speed");
  await expect(sizes).toContainText("Banked for");
  await expect(sizes.getByRole("button")).toHaveText(["58 km/h", "72 km/h", "86 km/h", "101 km/h", "115 km/h"]);
  await expect(sizes.getByRole("button").first()).toHaveAccessibleName("Banked for 58 km/h");
  await expect(sizes.locator('[aria-pressed="true"]')).toHaveCount(1);
  await expect(page.locator("[data-speed-match-message]")).toHaveText(/^Banked for \d+ km\/h suits the predicted entry speed\.$/);
  await expect(page.getByRole("region", { name: "Chosen piece" }).locator("[data-preview-name]")).toHaveText(/^Large left turn banked for \d+ km\/h$/);
  await expect(forces).toContainText("within rated range");
  await expect(forces).toContainText(/Estimated lateral force: 0(\.\d)? g \((weak|good|great|fine)\)/);
  await page.mouse.move(0, 0);
  await expect(page.locator(".builder__next")).toHaveScreenshot("speed-matched-turn-selection.png", { animations: "disabled" });

  const speedMatchedTurn = await selected.getAttribute("data-selected");
  await page.getByRole("button", { name: "Bank right" }).click();
  await expect(selected).toHaveAttribute("data-selected", "TurnLargeStandalone");
  await expect(bank).toHaveText("Level");
  await expect(forces).toContainText(/Estimated lateral force: [1-9](\.\d)? g/);
  await page.getByRole("button", { name: "Bank left" }).click();
  await expect(selected).toHaveAttribute("data-selected", speedMatchedTurn!);
  await expect(bank).toHaveText("For speed");

  await sizes.getByRole("button", { name: "Banked for 58 km/h" }).click();
  await expect(selected).toHaveAttribute("data-selected", "SpeedTurnLarge16Left");
  await placePiece.click();
  await expect(track).toHaveCount(7);
  await expect(track.nth(6).locator(".track-chip__name")).toHaveText("Large left turn banked for 58 km/h");
  await selectUnits(page, "imperial");
  await expect(track.nth(6).locator(".track-chip__name")).toHaveText("Large left turn banked for 36 mph");
});
