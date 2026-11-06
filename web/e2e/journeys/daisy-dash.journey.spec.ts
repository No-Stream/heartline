import { expect, test } from "@playwright/test";
import { readCircuit } from "../player/circuits";
import { closeCircuit, legacyRideRating, openTool, placeStation, setGameSpeed, startPark, testRide, waitForState } from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { expectRideReportExplained } from "../player/ride-report";

test("Daisy Dash family coaster journey @journey", async ({ page }, testInfo) => {
  test.setTimeout(240_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readCircuit("Daisy Dash");
  const chainOn = circuit.indexOf("+chain");
  const chainOff = circuit.indexOf("-chain");
  expect(circuit.slice(chainOn, chainOff)).toContain("TurnSmallStandalone");
  expect(circuit.filter((piece) => piece.startsWith("FamilyHill")).length).toBeGreaterThanOrEqual(2);
  expect(circuit.filter((piece) => piece.startsWith("SpeedTurnLarge")).length).toBeGreaterThanOrEqual(3);

  await startPark(page, { park: "new", seed: 2027, speed: 1 });
  await placeStation(page, { model: "family_coaster", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await expect(page.getByRole("combobox", { name: "Coaster model" }).locator("option:checked")).toHaveText("Daisy Dash · Windmill Ride Works");
  await placeCircuit(page, circuit);
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");

  await page.setViewportSize({ width: 1280, height: 720 });
  await closeCircuit(page);
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await testRide(page);
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  const { rating } = await expectRideReportExplained(page);
  console.log(`Daisy Dash ratings before/after: ${JSON.stringify({ before: legacyRideRating(rating), after: rating })}`);
  expect(rating.excitement).toBeGreaterThanOrEqual(2.5);
  expect(rating.excitement).toBeLessThanOrEqual(4.5);
  const report = await page.evaluate(() => window.gameTest.testingReport()!);
  expect(report.notes.join(" ")).not.toMatch(/could not finish|rolled back|rated speed band/i);
  expect(report.statistics.inversions).toBe(0);
  expect(report.statistics.launches).toBe(0);
  expect(report.statistics.maxPositiveVerticalG).toBeLessThanOrEqual(3);
  expect(report.statistics.maxNegativeVerticalG).toBeGreaterThanOrEqual(-0.6);
  expect(report.statistics.maxLateralG).toBeLessThanOrEqual(1);

  await expect(page.locator("[data-testing]")).toContainText("Scores (about 0–10)");
  await page.screenshot({ path: testInfo.outputPath("daisy-dash-finished-ride.png") });
  expect(pageErrors).toEqual([]);
});
