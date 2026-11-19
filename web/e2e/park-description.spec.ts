import { expect, test } from "@playwright/test";
import { openTool } from "./game-page";

test("a bundled park description loads and guests ride its coaster @slow", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?park=horizon-hawk&test=1&agents=12");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);

  const initialState = await page.evaluate(() => ({
    facilities: window.gameTest.facilities().map(({ kind }) => kind),
    paths: window.gameTest.paths(),
  }));
  expect(initialState.facilities).toContain("entrance");
  expect(initialState.paths.length).toBeGreaterThan(0);
  expect(initialState.paths.some(({ kind }) => kind === 1), "the loaded coaster queue should be present").toBe(true);

  let riderCount = 0;
  for (let simulatedTicks = 0; simulatedTicks < 30_000 && riderCount === 0; simulatedTicks += 1_000) {
    await page.evaluate(() => window.gameTest.advanceTicks(1_000));
    riderCount = await page.evaluate(() => window.gameTest.riders().length);
  }
  expect(riderCount, "a guest should board the coaster loaded from the park description").toBeGreaterThan(0);

  await openTool(page, "Watch");
  await page.getByRole("button", { name: "Export session" }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toContainText("Session export is unavailable after loading a park");
});
