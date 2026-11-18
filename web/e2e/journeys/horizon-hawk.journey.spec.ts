import { expect, test } from "@playwright/test";
import { readCircuit } from "../player/circuits";
import { closeCircuit, legacyRideRating, openTool, placeStation, setGameSpeed, startPark, testRide, waitForState } from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { expectRideReportExplained } from "../player/ride-report";

test("Horizon Hawk hyper coaster journey @journey", async ({ page }, testInfo) => {
  test.setTimeout(360_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readCircuit("Horizon Hawk");
  expect(circuit).toContain("WideLevelToSixtyDown");
  expect(circuit).toContain("FloaterHill24x160");
  expect(circuit).toContain("FloaterHill20x128");
  expect(circuit.filter((piece) => /^FloaterHill\d+x\d+$/.test(piece)).length).toBeGreaterThanOrEqual(4);
  expect(circuit.filter((piece) => piece.startsWith("SpeedTurnLarge")).length).toBeGreaterThanOrEqual(4);

  await startPark(page, { park: "new", seed: 2028, speed: 1 });
  await placeStation(page, { model: "hyper", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await expect(page.getByRole("combobox", { name: "Coaster model" }).locator("option:checked")).toHaveText("Horizon Hawk · Boulder & Marlin");

  // The lift is straight only, so the chain refuses a turn.
  const chain = page.getByRole("button", { name: "Chain lift", exact: true });
  await chain.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "true");
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  const turn = page.locator('[data-piece="TurnSmallStandalone"]');
  await expect(turn).toHaveClass(/piece-tile--refused/);
  await turn.click();
  await expect(page.locator("[data-construction] [data-message]")).toHaveText("No chain lift on this piece");
  await expect(page.getByRole("button", { name: "Place piece" })).toBeDisabled();
  await chain.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");

  await placeCircuit(page, circuit);
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");

  await page.setViewportSize({ width: 1280, height: 720 });
  await closeCircuit(page);
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await testRide(page);
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 40_000 });
  const { rating } = await expectRideReportExplained(page);
  console.log(`Horizon Hawk ratings before/after: ${JSON.stringify({ before: legacyRideRating(rating), after: rating })}`);
  expect(rating.excitement).toBeGreaterThanOrEqual(6);
  expect(rating.excitement).toBeLessThanOrEqual(8);
  const report = await page.evaluate(() => window.gameTest.testingReport()!);
  expect(report.notes.join(" ")).not.toMatch(/could not finish|rolled back|rated speed band/i);
  expect(report.statistics.inversions).toBe(0);
  expect(report.statistics.launches).toBe(0);
  expect(report.statistics.maxPositiveVerticalG).toBeLessThanOrEqual(4);
  expect(report.statistics.maxNegativeVerticalG).toBeGreaterThanOrEqual(-1.3);
  expect(report.statistics.maxLateralG).toBeLessThanOrEqual(1.2);

  await expect(page.locator("[data-testing]")).toContainText("Scores (about 0–10)");
  await page.screenshot({ path: testInfo.outputPath("horizon-hawk-finished-ride.png") });
  expect(pageErrors).toEqual([]);
});
