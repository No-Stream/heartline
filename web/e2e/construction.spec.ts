import { expect, test } from "@playwright/test";
import { canvasSize, clickCanvas, moveOverCanvas, openTool } from "./game-page";

test("builds a closed coaster and runs a train around it", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
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
    "LiftLevelToGentleUp", "Lift", "Lift", "Lift", "Lift", "Lift", "LiftGentleUpToLevel",
    "LiftFlat", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit",
    "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown",
    "GentleDownToLevel", "Loop", "TurnLargeEntry", "TurnLargeExit",
  ];
  for (const [index, piece] of pieces.entries()) {
    await choices.locator(`[data-piece="${piece}"]`).click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    if (index === 0) {
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      await page.addStyleTag({ content: "[data-testid='tick-count'] { visibility: hidden; }" });
      await page.mouse.move(0, 0);
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
  await expect(page.locator("[data-notes]")).toHaveText(notes.join(" "));
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
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
  await expect(page.getByText("30 m/s+")).toBeVisible();
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
  await expect(page.locator("[data-testing]")).toHaveScreenshot("coaster-testing-panel.png", { animations: "disabled" });
  await expect(canvas).toHaveScreenshot("coaster-testing-overlay.png", { animations: "disabled" });
  await page.getByLabel("Track overlay").selectOption("airtime");
  await expect(page.getByText("No airtime")).toBeVisible();
  await page.getByRole("button", { name: "Delete Begin chain lift climb, piece 2" }).click();
  await expect(track.locator("li")).toHaveCount(pieces.length, { timeout: 30_000 });
  await expect(track.locator("li").last()).toContainText("End large left turn");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeDisabled();
  await expect(page.getByText("Complete the circuit and make it longer than the train to test your coaster.")).toBeVisible();
  await expect(page.getByLabel("Track overlay")).toBeDisabled();
});
