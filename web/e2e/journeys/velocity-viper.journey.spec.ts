import { expect, test } from "@playwright/test";
import { readCircuit } from "../player/circuits";
import { placeCircuit } from "../player/place-circuit";
import { legacyRideRating, openTool, placeStation, setGameSpeed, startPark, waitForState } from "../player/driver";
import { expectRideReportExplained } from "../player/ride-report";

test("Velocity Viper keyboard construction journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readCircuit("Velocity Viper");
  const topHatIndex = circuit.indexOf("LaunchedTopHat");
  expect(topHatIndex).toBeGreaterThan(0);
  expect(circuit[topHatIndex - 1]).toMatch(/^Launch/);
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await placeStation(page, { model: "velocity_viper", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await placeCircuit(page, circuit, true);
  await page.locator("canvas").focus();
  await page.keyboard.press("c");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await page.locator("canvas").focus();
  const crestCount = await page.evaluate(() => window.gameTest.crestBursts().length);
  await page.keyboard.press("t");
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  const { rating } = await expectRideReportExplained(page);
  expect(rating.excitement).toBeGreaterThanOrEqual(6);
  expect(rating.excitement).toBeLessThanOrEqual(9);
  expect((await page.evaluate(() => window.gameTest.testingReport()!)).statistics.launches).toBe(2);
  console.log(`Velocity Viper ratings before/after: ${JSON.stringify({ before: legacyRideRating(rating), after: rating })}`);
  await page.setViewportSize({ width: 1280, height: 1200 });
  const testingPanel = page.locator("[data-testing]");
  await expect(testingPanel).toContainText("Scores (about 0–10)");
  await page.evaluate(() => {
    const dock = document.querySelector<HTMLElement>("[data-dock]");
    const testing = document.querySelector<HTMLElement>("[data-testing]");
    if (!dock || !testing) throw new Error("Coaster testing panel is missing");
    dock.scrollTop += testing.getBoundingClientRect().top - dock.getBoundingClientRect().top;
  });
  const panelHeight = await testingPanel.evaluate((element) => Math.ceil(element.getBoundingClientRect().height));
  await page.setViewportSize({ width: 1280, height: Math.max(720, panelHeight + 100) });
  await expect(testingPanel).toHaveScreenshot("coaster-testing-panel.png", { animations: "disabled" });
  await page.setViewportSize({ width: 1280, height: 720 });
  const notes = await page.evaluate(() => window.gameTest.testingReport()!.notes);
  expect(notes.join(" ")).not.toMatch(/could not finish|rolled back/i);
  const launchPositions = page.getByRole("region", { name: "Launch positions" }).locator("li");
  await expect(launchPositions).toHaveCount(4);
  const launchCentres = (await launchPositions.allTextContents()).map((text) => {
    const match = /(\d+(?:\.\d+)?) m from station/.exec(text);
    if (!match) throw new Error(`Launch position has no distance: ${text}`);
    return Number(match[1]);
  });
  const launchSpeeds = await page.evaluate((centres) => {
    const samples = window.gameTest.testingReport()!.samples;
    return centres.map((centre) => samples.reduce((closest, sample) =>
      Math.abs(sample.distance - centre) < Math.abs(closest.distance - centre) ? sample : closest).speed);
  }, launchCentres);
  expect(launchSpeeds).toHaveLength(4);
  for (const speed of launchSpeeds) expect(speed).toBeGreaterThan(2);
  expect(await page.evaluate(() => window.gameTest.crestBursts().length)).toBeGreaterThan(crestCount);
});
