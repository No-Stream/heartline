import { expect, test } from "@playwright/test";
import { canvasSize, clickCanvas, clickTile, moveOverCanvas, openTool, selectUnits } from "./game-page";
test("station placement targets ground directly beneath a coaster support", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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
    await page.goto("/?test=1");
    await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
    await openTool(page, "Coaster");
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
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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

test("turns after a drop default to the bank for the predicted speed, and the bank stepper overrides it @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
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
  await expect(selected).toHaveAttribute("data-selected", /^SpeedTurnLarge(16|20|24)Left$/);
  await expect(bank).toHaveText("For speed");
  await expect(sizes).toContainText("Banked for");
  await expect(sizes.getByRole("button")).toHaveText(["58 km/h", "72 km/h", "86 km/h"]);
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
