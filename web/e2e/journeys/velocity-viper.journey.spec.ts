import { expect, test } from "@playwright/test";
import { readCircuit } from "../player/circuits";
import { legacyRideRating, openTool, placeStation, setGameSpeed, startPark, waitForState } from "../player/driver";
import { expectRideReportExplained } from "../player/ride-report";

test("Velocity Viper keyboard construction journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readCircuit("Velocity Viper");
  const topHatIndex = circuit.indexOf("LaunchedTopHat");
  expect(topHatIndex).toBeGreaterThan(0);
  expect(circuit[topHatIndex - 1]).toMatch(/^Launch/);
  await startPark(page, { park: "new", seed: 2026, speed: 1, livePauseRender: true });
  await placeStation(page, { model: "velocity_viper", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await page.locator("canvas").focus();
  const selected = page.locator('[data-piece][aria-checked="true"]');
  const selectWithKeyboard = async (piece: string): Promise<void> => {
    for (let step = 0; await selected.getAttribute("data-piece") !== piece; step++) {
      if (step >= 100) throw new Error(`Piece ${piece} is not offered by the keyboard palette`);
      const previous = await selected.getAttribute("data-piece");
      await page.keyboard.press("]");
      await expect(selected).not.toHaveAttribute("data-piece", previous!);
    }
  };
  let count = 1;
  for (const piece of circuit) {
    if (piece === "+chain" || piece === "-chain") {
      await page.locator("canvas").focus();
      await page.keyboard.press("h");
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(piece === "+chain"));
      continue;
    }
    if (piece === "TurnSmallStandalone" && count === 3) {
      await selectWithKeyboard("GentleUp");
      await page.keyboard.press("a");
      await expect(selected, "turn left should offer the climbing turn").toHaveAttribute("data-piece", piece);
    } else if (piece === "GentleUpToLevel") {
      await selectWithKeyboard("GentleUp");
      await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Gentle up 27°");
      await page.keyboard.press("s");
      await expect(selected, "slope down should select the level-out piece").toHaveAttribute("data-piece", piece);
      await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Level");
    } else if (piece === "GentleDownToLevel") {
      await selectWithKeyboard("GentleDown");
      await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Gentle down 27°");
      await page.keyboard.press("w");
      await expect(selected, "slope up should select the level-out piece").toHaveAttribute("data-piece", piece);
      await expect(page.locator('[data-stepper-value="slope"]')).toHaveText("Level");
    } else {
      await selectWithKeyboard(piece);
    }
    await expect(page.getByRole("button", { name: "Place piece" }), `cannot place ${piece}`).toBeEnabled();
    await page.keyboard.press("b");
    await expect(page.locator("[data-track] li")).toHaveCount(++count);
  }
  await page.keyboard.press("c");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await page.locator("canvas").focus();
  const crestCount = await page.evaluate(() => window.gameTest.crestBursts().length);
  await page.keyboard.press("t");
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  const { rating } = await expectRideReportExplained(page);
  console.log(`Velocity Viper ratings before/after: ${JSON.stringify({ before: legacyRideRating(rating), after: rating })}`);
  await page.locator("canvas").focus();
  await page.keyboard.press("Space");
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
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
  await expect(launchPositions).toHaveCount(3);
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
  expect(launchSpeeds).toHaveLength(3);
  for (const speed of launchSpeeds) expect(speed).toBeGreaterThan(2);
  expect(await page.evaluate(() => window.gameTest.crestBursts().length)).toBeGreaterThan(crestCount);
});
