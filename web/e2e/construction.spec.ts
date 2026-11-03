import { expect, test } from "@playwright/test";

test("builds a closed coaster and runs a train around it", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").selectOption("72");
  await page.getByRole("button", { name: "Place station" }).click();
  const position = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.mouse.move(bounds.x + position.x * bounds.width / 800, bounds.y + position.y * bounds.height / 500);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50,80,0/);
  await page.mouse.click(bounds.x + position.x * bounds.width / 800, bounds.y + position.y * bounds.height / 500);
  const north = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 52.5, z: 2 }));
  await page.mouse.move(bounds.x + north.x * bounds.width / 800, bounds.y + north.y * bounds.height / 500);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50,80,1/);
  const direction = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await page.mouse.move(bounds.x + direction.x * bounds.width / 800, bounds.y + direction.y * bounds.height / 500);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50,80,0/);
  await page.mouse.click(bounds.x + direction.x * bounds.width / 800, bounds.y + direction.y * bounds.height / 500);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 30, centerY: 30, zoom: 1, rotation: 0 }));
  const choices = page.getByLabel("Next piece");
  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  const pieces = [
    "LiftLevelToGentleUp", "Lift", "Lift", "Lift", "Lift", "Lift", "LiftGentleUpToLevel",
    "LiftFlat", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit",
    "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown",
    "GentleDownToLevel", "Loop", "TurnLargeEntry", "TurnLargeExit",
  ];
  for (const [index, piece] of pieces.entries()) {
    await expect(choices.locator(`option[value="${piece}"]`)).toHaveCount(1);
    await choices.selectOption(piece);
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    if (index === 0) {
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      await page.addStyleTag({ content: "[data-testid='tick-count'] { visibility: hidden; }" });
      await expect(page.locator("main")).toHaveScreenshot("construction-preview.png", { animations: "disabled" });
    }
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await page.getByRole("button", { name: "Test coaster" }).click();
  await page.evaluate(() => window.gameTest.advanceTicks(10000));
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status")).toHaveText("Train completed a lap.", { timeout: 150_000 });
  await expect.poll(() => page.evaluate(() => window.gameTest.testingReport()?.samples.length ?? 0)).toBeGreaterThan(1);
  const notes = await page.evaluate(() => window.gameTest.testingReport()!.notes);
  expect(notes.length).toBeGreaterThan(0);
  await expect(page.locator("[data-notes]")).toHaveText(notes.join(" "));
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  await page.getByLabel("Track overlay").selectOption("speed");
  await page.evaluate(() => {
    const samples = window.gameTest.testingReport()!.samples;
    const horizontal = samples.map((sample) => sample.x - sample.y);
    const vertical = samples.map((sample) => sample.x + sample.y - 2 * sample.z);
    const middleX = (Math.min(...horizontal) + Math.max(...horizontal)) / 2;
    const middleY = (Math.min(...vertical) + Math.max(...vertical)) / 2;
    const zoom = Math.min(0.7, 700 / (32 * (Math.max(...horizontal) - Math.min(...horizontal))), 400 / (16 * (Math.max(...vertical) - Math.min(...vertical))));
    window.gameTest.setCamera({ centerX: (middleX + middleY) / 2, centerY: (middleY - middleX) / 2, zoom, rotation: 0 });
  });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready), { timeout: 30_000 }).toBe(true);
  await expect(page.getByText("30 m/s+")).toBeVisible();
  const inspectionPoint = await page.evaluate(() => {
    const report = window.gameTest.testingReport();
    if (!report) throw new Error("Missing coaster test report");
    for (const sample of report.samples) {
      const position = window.gameTest.project(sample);
      if (position.x < 0 || position.x >= 800 || position.y < 0 || position.y >= 500) continue;
      const pick = window.gameTest.pick(position.x, position.y);
      if (pick?.kind === "testing") return position;
    }
    throw new Error("No testing overlay segment was pickable");
  });
  await canvas.scrollIntoViewIfNeeded();
  const overlayBounds = await canvas.boundingBox();
  if (!overlayBounds) throw new Error("Canvas is not visible");
  await page.mouse.click(overlayBounds.x + inspectionPoint.x * overlayBounds.width / 800, overlayBounds.y + inspectionPoint.y * overlayBounds.height / 500);
  await expect(page.locator("[data-inspection]")).toContainText("Vertical:");
  await expect(page.locator("[data-testing]")).toHaveScreenshot("coaster-testing-panel.png", { animations: "disabled" });
  await expect(canvas).toHaveScreenshot("coaster-testing-overlay.png", { animations: "disabled" });
  await page.getByLabel("Track overlay").selectOption("airtime");
  await expect(page.getByText("No airtime")).toBeVisible();
  await page.getByRole("button", { name: "Delete LiftLevelToGentleUp, piece 2" }).click();
  await expect(track.locator("li")).toHaveCount(pieces.length, { timeout: 30_000 });
  await expect(track.locator("li").last()).toContainText("TurnLargeExit");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeDisabled();
  await expect(page.getByText("Complete the circuit and make it longer than the train to test your coaster.")).toBeVisible();
  await expect(page.getByLabel("Track overlay")).toBeDisabled();
});
