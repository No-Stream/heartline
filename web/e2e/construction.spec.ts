import { expect, test } from "@playwright/test";
import { canvasSize, clickCanvas, moveOverCanvas, openTool, selectUnits } from "./game-page";
import { formatTestingNote } from "../src/coaster-testing";

function displayedHeight(text: string, unit: "ft" | "m"): number {
  const match = text.match(/(\d+(?:\.\d+)?)\s*(ft|m)\b/);
  if (!match || match[2] !== unit) throw new Error(`Expected a height in ${unit}, received: ${text}`);
  return Number(match[1]);
}

test("builds a closed coaster and runs a train around it @slow", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  const stationHeightInput = page.getByLabel("Station height above ground (metres)");
  await expect(stationHeightInput).toHaveValue("12");
  await page.getByRole("button", { name: "Raise the station" }).click();
  await expect(stationHeightInput).toHaveValue("14");
  await selectUnits(page, "imperial");
  await expect(page.getByLabel("Station height above ground (feet)")).toHaveValue("45.93");
  await selectUnits(page, "metric");
  await expect(stationHeightInput).toHaveValue("14");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await stationHeightInput.fill("72");
  await stationHeightInput.press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  const position = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  const canvas = page.locator("canvas");
  await moveOverCanvas(page, position);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50,80,0/);
  await clickCanvas(page, position);
  const north = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 52.5, z: 2 }));
  await moveOverCanvas(page, north);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50,80,1/);
  const direction = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, direction);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50,80,0/);
  await clickCanvas(page, direction);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 30, centerY: 30, zoom: 1, rotation: 0 }));
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  const pieces = [
    "LevelToGentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUpToLevel",
    "Straight", "OneTileStraight", "OneTileStraight", "OffsetLeft", "TurnLargeEntry", "TurnLargeExit",
    "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown",
    "GentleDownToLevel", "Loop", "OffsetRight", "TurnLargeEntry", "TurnLargeExit",
    "Straight", "OneTileStraight", "OneTileStraight",
  ];
  for (const [index, piece] of pieces.entries()) {
    if (index === 0 || index === 8) await page.keyboard.press("h");
    await choices.locator(`[data-piece="${piece}"]`).click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    if (index === 0) {
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      await page.addStyleTag({ content: "[data-testid='tick-count'] { visibility: hidden; }" });
      await page.mouse.move(0, 0);
      await page.evaluate(() => {
        const dock = document.querySelector<HTMLElement>("[data-dock]");
        if (!dock) throw new Error("Tool panel is missing");
        dock.scrollTop = 0;
      });
      await expect(page.locator("main")).toHaveScreenshot("construction-preview.png", { animations: "disabled" });
    }
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
    if (index === 0) {
      // The camera started far away at (30, 30); it follows the open track end back to the station.
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      const station = await page.evaluate(() => window.gameTest.project({ x: 52, y: 50.5, z: 20 }));
      const size = await canvasSize(page);
      expect(station.x).toBeGreaterThan(0);
      expect(station.x).toBeLessThan(size.width);
      expect(station.y).toBeGreaterThan(0);
      expect(station.y).toBeLessThan(size.height);
    }
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await page.getByRole("button", { name: "Test coaster" }).click();
  await page.evaluate(() => window.gameTest.advanceTicks(10000));
  expect(await page.getByRole("region", { name: "Coaster construction" }).getByRole("status").textContent()).toBe("Train completed a lap.");
  const testingReport = await page.evaluate(() => window.gameTest.testingReport());
  expect(testingReport?.samples.length ?? 0).toBeGreaterThan(1);
  const notes = await page.evaluate(() => window.gameTest.testingReport()!.notes);
  expect(notes.length).toBeGreaterThan(0);
  await expect(page.locator("[data-notes]")).toHaveText(notes.map((note) => formatTestingNote(note, "metric")).join(" "));
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  const maximumHeight = page.locator("[data-report-maximum-height]");
  const biggestDrop = page.locator("[data-report-drop-height]");
  const metricMaximumHeight = displayedHeight((await maximumHeight.textContent()) ?? "", "m");
  const metricBiggestDrop = displayedHeight((await biggestDrop.textContent()) ?? "", "m");
  await selectUnits(page, "imperial");
  const imperialMaximumHeight = displayedHeight((await maximumHeight.textContent()) ?? "", "ft");
  const imperialBiggestDrop = displayedHeight((await biggestDrop.textContent()) ?? "", "ft");
  expect(metricMaximumHeight).toBeCloseTo(imperialMaximumHeight * 0.3048, 1);
  expect(metricBiggestDrop).toBeCloseTo(imperialBiggestDrop * 0.3048, 1);
  await selectUnits(page, "metric");
  const proximityEventCount = await page.evaluate(() => window.gameTest.testingReport()!.proximityEvents.length);
  expect(proximityEventCount).toBeGreaterThan(0);
  await expect(page.locator("[data-proximity-events]")).toContainText(`Proximity events (${proximityEventCount})`);
  await expect(page.locator("[data-proximity-events]")).toContainText("excitement from proximity events");
  await page.getByLabel("Track overlay").selectOption("speed");
  const overlaySize = await canvasSize(page);
  await page.evaluate((size) => {
    const samples = window.gameTest.testingReport()!.samples;
    const horizontal = samples.map((sample) => sample.x - sample.y);
    const vertical = samples.map((sample) => sample.x + sample.y - 2 * sample.z);
    const middleX = (Math.min(...horizontal) + Math.max(...horizontal)) / 2;
    const middleY = (Math.min(...vertical) + Math.max(...vertical)) / 2;
    const zoom = Math.min(0.7, (size.width - 100) / (32 * (Math.max(...horizontal) - Math.min(...horizontal))), (size.height - 100) / (16 * (Math.max(...vertical) - Math.min(...vertical))));
    window.gameTest.setCamera({ centerX: (middleX + middleY) / 2, centerY: (middleY - middleX) / 2, zoom, rotation: 0 });
  }, overlaySize);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready), { timeout: 30_000 }).toBe(true);
  const speedHigh = page.locator("[data-high]");
  await expect(speedHigh).toHaveText("108 km/h+");
  await selectUnits(page, "imperial");
  await expect(speedHigh).toHaveText("67.1 mph+");
  await selectUnits(page, "metric");
  await expect(speedHigh).toHaveText("108 km/h+");
  const proximityInspectionPoint = await page.evaluate((size) => {
    const report = window.gameTest.testingReport();
    if (!report) throw new Error("Missing coaster test report");
    for (const [index, event] of report.proximityEvents.entries()) {
      const position = window.gameTest.project(event);
      if (position.x < 0 || position.x >= size.width || position.y < 0 || position.y >= size.height) continue;
      const pick = window.gameTest.pick(position.x, position.y);
      if (pick?.kind === "testing" && pick.id === `proximity-${index}`) return { position, index };
    }
    throw new Error("No proximity marker was pickable");
  }, overlaySize);
  await clickCanvas(page, proximityInspectionPoint.position);
  await expect(page.locator("[data-inspection]")).toContainText(/piece .*Strength:/);
  const inspectionPoint = await page.evaluate((size) => {
    const report = window.gameTest.testingReport();
    if (!report) throw new Error("Missing coaster test report");
    for (const sample of report.samples) {
      const position = window.gameTest.project(sample);
      if (position.x < 0 || position.x >= size.width || position.y < 0 || position.y >= size.height) continue;
      const pick = window.gameTest.pick(position.x, position.y);
      if (pick?.kind === "testing" && /^testing-\d+$/.test(pick.id)) return position;
    }
    throw new Error("No testing overlay segment was pickable");
  }, overlaySize);
  await clickCanvas(page, inspectionPoint);
  await expect(page.locator("[data-inspection]")).toContainText("Vertical:");
  await page.evaluate(() => {
    const dock = document.querySelector<HTMLElement>("[data-dock]");
    const testing = document.querySelector<HTMLElement>("[data-testing]");
    if (!dock || !testing) throw new Error("Coaster testing panel is missing");
    dock.scrollTop += testing.getBoundingClientRect().top - dock.getBoundingClientRect().top;
  });
  await expect(page.locator("[data-testing]")).toHaveScreenshot("coaster-testing-panel.png", { animations: "disabled" });
  await expect(canvas).toHaveScreenshot("coaster-testing-overlay.png", { animations: "disabled" });
  await page.getByLabel("Track overlay").selectOption("airtime");
  await expect(page.getByText("No airtime")).toBeVisible();
  await page.getByRole("button", { name: "Delete Begin gentle climb, piece 2" }).click();
  await expect(track.locator("li")).toHaveCount(pieces.length, { timeout: 30_000 });
  await expect(track.locator("li").last()).toContainText("One-tile straight");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeDisabled();
  await expect(page.getByText("Complete the circuit and make it longer than the train to test your coaster.")).toBeVisible();
  await expect(page.getByLabel("Track overlay")).toBeDisabled();
});

test("shows and places visible slope-to-level choices on Velocity Viper @slow", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:velocity_viper");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  const station = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, station);
  await clickCanvas(page, station);
  const direction = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, direction);
  await clickCanvas(page, direction);

  const palette = page.getByRole("radiogroup", { name: "Next piece" });
  const place = async (pieceId: string): Promise<void> => {
    await palette.locator(`[data-piece="${pieceId}"]`).click();
    await page.getByRole("button", { name: "Place piece" }).click();
  };

  await place("LevelToGentleUp");
  await place("GentleUp");
  const upLevelOut = palette.locator('[data-piece="GentleUpToLevel"]');
  await expect(upLevelOut).toBeVisible();
  await expect(upLevelOut).toContainText(/level/i);
  await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Gentle up 27°");
  await page.getByRole("button", { name: "Slope down" }).click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", "GentleUpToLevel");
  await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Level");
  await page.getByRole("button", { name: "Place piece" }).click();
  await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Level");

  await place("LevelToGentleDown");
  await place("GentleDown");
  const downLevelOut = palette.locator('[data-piece="GentleDownToLevel"]');
  await expect(downLevelOut).toBeVisible();
  await expect(downLevelOut).toContainText(/level/i);
  await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Gentle down 27°");
  await page.getByRole("button", { name: "Slope up" }).click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", "GentleDownToLevel");
  await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Level");
  await page.getByRole("button", { name: "Place piece" }).click();
  await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Level");
});

test("builds a descending banked turn on Briar Blitz from the visible construction choices", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.locator(".builder select[data-model]").selectOption("model:wooden_twister");
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  const station = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, station);
  await clickCanvas(page, station);
  const direction = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, direction);
  await clickCanvas(page, direction);

  const palette = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track] li");
  await expect(track).toHaveCount(1);
  let expectedTrackCount = 1;
  const place = async (pieceId: string): Promise<void> => {
    const choice = palette.locator(`[data-piece="${pieceId}"]`);
    await expect(choice).toBeVisible();
    await choice.click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", pieceId);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    expectedTrackCount += 1;
    await expect(track).toHaveCount(expectedTrackCount);
  };

  await place("LevelToGentleDown");
  await place("GentleDown");
  await place("BankLeft");
  await place("BankedTurnStandalone");
  await place("UnbankLeft");
  await expect(track).toHaveCount(6);
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
  await expect(closeCircuit).toBeDisabled();
  for (const [index, piece] of pieces.entries()) {
    const choice = choices.locator(`[data-piece="${piece}"]`);
    await expect(choice).toBeVisible();
    await choice.click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
    if (index < pieces.length - 1) await expect(closeCircuit).toBeDisabled();
  }

  await expect(track.last()).toContainText("Standalone small left turn");
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
    "Straight",
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
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
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
});
