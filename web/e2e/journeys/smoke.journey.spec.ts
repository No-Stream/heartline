import { expect, test } from "@playwright/test";
import { layPath, placeFacility, startPark, waitForState } from "../player/driver";
import { revenue } from "../player/state";

test("guests buy food in a new park @journey", async ({ page }) => {
  test.setTimeout(60_000);
  await startPark(page, { park: "new", seed: 2026, speed: 16 });
  const entrance = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "entrance"));
  expect(entrance).toBeDefined();
  expect(entrance?.price).toBe(0);
  expect(await page.evaluate(() => window.gameTest.facilities().length)).toBe(1);

  await layPath(page, { x: 218, y: 251, z: 2 }, { x: 218, y: 256, z: 2 });
  await placeFacility(page, "Food stall", { x: 216, y: 252, z: 2 });
  const facilities = await page.evaluate(() => window.gameTest.facilities());
  expect(facilities.map(({ kind }) => kind).sort()).toEqual(["entrance", "food_stall"]);
  expect(facilities.find(({ kind }) => kind === "food_stall")?.price).toBeGreaterThan(0);
  const initialRevenue = await revenue(page);
  await waitForState(page, async () => (await revenue(page)) > initialRevenue, { ticks: 30_000 });
});
