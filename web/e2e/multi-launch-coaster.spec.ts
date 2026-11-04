import { expect, test } from "@playwright/test";
import { clickCanvas, moveOverCanvas, openTool, selectUnits } from "./game-page";

test.use({ launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] } });

test("builds and runs the Velocity Viper with two short launches @slow", async ({ page }) => {
  test.setTimeout(240_000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");

  const model = page.getByRole("combobox", { name: "Coaster model" });
  await model.selectOption("model:velocity_viper");
  await expect(model).toHaveValue("model:velocity_viper");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").fill("12");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();

  const canvas = page.locator("canvas");
  const stationPosition = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, stationPosition);
  await expect(canvas).toHaveAttribute("data-station-preview", /50,50/);
  await clickCanvas(page, stationPosition);
  const east = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, east);
  await clickCanvas(page, east);

  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  const pieces = [
    "Short launch",
    "Standalone small left turn",
    "Standalone small left turn",
    "Short launch",
    "Straight",
    "Standalone small left turn",
    "Standalone small left turn",
  ];
  for (const [index, piece] of pieces.entries()) {
    await page.getByRole("radio", { name: piece, exact: true }).click();
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
    await expect(track.locator("li").last()).toContainText(piece);
  }

  const launchPositions = page.getByRole("region", { name: "Launch positions" }).locator("li");
  await expect(launchPositions).toHaveCount(2);
  await expect(launchPositions.nth(0)).toContainText("Piece 2");
  await expect(launchPositions.nth(1)).toContainText("Piece 5");
  await expect(launchPositions.nth(1)).toContainText(/m from station/);
  const launchPositionText = await launchPositions.allTextContents();
  const launchCentres = launchPositionText.map((text) => {
    const match = /(\d+(?:\.\d+)?) m from station/.exec(text);
    if (!match) throw new Error(`Launch position has no distance: ${text}`);
    return Number(match[1]);
  });
  expect(launchCentres[1]).toBeGreaterThan(launchCentres[0]);

  await page.getByRole("button", { name: "Close circuit" }).click();
  const testCoaster = page.getByRole("button", { name: "Test coaster" });
  await expect(testCoaster).toBeEnabled();
  await testCoaster.click();
  await page.evaluate(() => window.gameTest.advanceTicks(10_000));
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status"))
    .toHaveText("Train completed a lap.", { timeout: 150_000 });
  await expect.poll(() => page.evaluate(() => window.gameTest.testingReport()?.samples.length ?? 0)).toBeGreaterThan(1);
  await page.getByLabel("Track overlay").selectOption("speed");
  await expect(page.getByText("108 km/h+")).toBeVisible();
  await expect(launchPositions).toHaveCount(2);
  await expect(launchPositions.nth(0)).toBeVisible();
  const launchSpeeds = await page.evaluate((centres) => {
    const samples = window.gameTest.testingReport()?.samples;
    if (!samples) throw new Error("Missing coaster test samples");
    return centres.map((centre) => samples.reduce((closest, sample) =>
      Math.abs(sample.distance - centre) < Math.abs(closest.distance - centre) ? sample : closest).speed);
  }, launchCentres);
  expect(launchSpeeds).toHaveLength(2);
  for (const speed of launchSpeeds) expect(speed).toBeGreaterThan(2);
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
  const launchOverlayPicks = await page.evaluate((centres) => centres.map((centre) => {
    const report = window.gameTest.testingReport();
    if (!report) throw new Error("Missing coaster test report");
    const sample = report.samples.reduce((closest, candidate) =>
      Math.abs(candidate.distance - centre) < Math.abs(closest.distance - centre) ? candidate : closest);
    const point = window.gameTest.project(sample);
    for (let dy = -3; dy <= 3; dy++) for (let dx = -3; dx <= 3; dx++) {
      if (window.gameTest.pick(point.x + dx, point.y + dy)?.kind === "testing") return true;
    }
    return false;
  }), launchCentres);
  expect(launchOverlayPicks).toEqual([true, true]);
  expect(errors).toEqual([]);
});
