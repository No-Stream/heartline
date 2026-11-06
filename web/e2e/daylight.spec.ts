import { expect, test, type Page } from "@playwright/test";
import { openTool } from "./game-page";
import { browserLaunchOptions } from "./fixtures/browser-launch-options";

test.use({
  viewport: { width: 1280, height: 750 },
  deviceScaleFactor: 1,
  launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]),
});

/** 21:42, when a firework from the fixed test-mode schedule is in full bloom over the demo park. */
const FIREWORK_NIGHT = 0.904167;

async function startPaused(page: Page): Promise<void> {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
}

async function showDemoParkAt(page: Page, phase: number): Promise<void> {
  await page.evaluate((timeOfDay) => {
    window.gameTest.setCamera({ centerX: 236, centerY: 250, zoom: 0.75, rotation: 0 });
    window.gameTest.setTimeOfDay(timeOfDay);
  }, phase);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
}

test.describe("day and night", () => {
  for (const [name, phase] of [["noon", 0.5], ["dusk", 0.76], ["night", FIREWORK_NIGHT]] as const) {
    test(`demo park at ${name}`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await startPaused(page);
      await showDemoParkAt(page, phase);
      await expect(page.locator("canvas")).toHaveScreenshot(`demo-park-${name}.png`, { animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001 });
      expect(errors).toEqual([]);
    });
  }

  test("sky beyond the park edge at night", async ({ page }) => {
    await startPaused(page);
    await page.evaluate(() => {
      window.gameTest.setCamera({ centerX: 4, centerY: 4, zoom: 0.5, rotation: 0 });
      window.gameTest.setTimeOfDay(0.95);
    });
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    await expect(page.locator("canvas")).toHaveScreenshot("park-edge-night.png", { animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001 });
  });

  test("surface lamps are hidden in the underground view at night", async ({ page }) => {
    await startPaused(page);
    await showDemoParkAt(page, 0.95);
    await openTool(page, "Landscape");
    const landscaping = page.getByRole("region", { name: "Landscaping" });
    await landscaping.getByLabel("Underground view style").selectOption("cut");
    await landscaping.getByLabel("Underground cut level").fill("4");
    await landscaping.locator("[data-underground-toggle]").click();
    await expect(landscaping.locator("[data-underground-toggle]")).toHaveText("Show surface");
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    await expect(page.locator("canvas")).toHaveScreenshot("demo-park-underground-night.png", {
      animations: "disabled",
      threshold: 0.1,
      maxDiffPixelRatio: 0.001,
    });
  });

  test("test mode holds noon until a test sets the time", async ({ page }) => {
    await startPaused(page);
    expect(await page.evaluate(() => window.gameTest.timeOfDay())).toBe(0.5);
    await page.evaluate(() => window.gameTest.advanceTicks(900));
    expect(await page.evaluate(() => window.gameTest.timeOfDay())).toBe(0.5);
    await expect(page.getByLabel("Time of day", { exact: true })).toHaveText("12:00");
  });

  test("the clock follows simulation ticks while the game runs", async ({ page }) => {
    await page.goto("/");
    await expect.poll(() => page.evaluate(() => Number(document.querySelector("[data-testid='tick-count']")?.textContent)), { timeout: 30_000 }).toBeGreaterThan(30);
    const read = () => page.evaluate(() => ({ phase: window.gameTest.timeOfDay(), tick: Number(document.querySelector("[data-testid='tick-count']")!.textContent) }));
    const before = await read();
    await expect.poll(async () => (await read()).tick, { timeout: 30_000 }).toBeGreaterThan(before.tick + 60);
    const after = await read();
    // One day is 43,200 ticks; the readout and the clock can be one snapshot apart.
    expect((after.phase - before.phase) * 43_200).toBeCloseTo(after.tick - before.tick, -1);
  });

  test("the Watch tool can scrub the day and change its speed @fast", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 750 });
    await startPaused(page);
    const scrub = page.getByRole("slider", { name: "Set the time of day" });
    await scrub.fill("1290");
    await expect(page.getByLabel("Time of day", { exact: true })).toHaveText("21:30");
    expect(await page.evaluate(() => window.gameTest.timeOfDay())).toBeCloseTo(1290 / 1440, 6);
    const speed = page.getByRole("button", { name: "Day speed" });
    await speed.click();
    await expect(speed).toHaveText("×10");
    await speed.click();
    await expect(speed).toHaveText("×60");
    await openTool(page, "Paths");
    await expect(scrub).toBeHidden();
    await openTool(page, "Watch");
    await expect(scrub).toBeVisible();
  });

  test("the demo coaster throws confetti once per lap at the top of its lift hill @fast", async ({ page }) => {
    test.setTimeout(30_000);
    await startPaused(page);
    // Any crest with a large enough climb throws confetti (lift hills and airtime hills alike), so only the lift crests are counted here:
    // the demo's lift crest is 46 m up (11.5 tiles) and its tallest airtime hill tops out near 34 m.
    const liftCrestBursts = () => page.evaluate(() => window.gameTest.crestBursts().filter((burst) => burst.z > 10));
    for (let advancedTicks = 0; advancedTicks < 30_000; advancedTicks += 100) {
      if ((await liftCrestBursts()).length >= 2) break;
      await page.evaluate(() => window.gameTest.advanceTicks(100));
    }
    const bursts = await liftCrestBursts();
    expect(bursts).toHaveLength(2);
    const ticksBetweenBursts = bursts[1].tick - bursts[0].tick;
    expect(ticksBetweenBursts).toBeGreaterThan(1_000);
    expect(ticksBetweenBursts).toBeLessThan(10_000);
  });
});
