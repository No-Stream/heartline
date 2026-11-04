import { expect, test } from "@playwright/test";
import { canvasToPage, moveOverCanvas, openTool } from "./game-page";

test("pausing freezes guests and ticks while tools remain usable", async ({ page }) => {
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const pause = page.getByRole("button", { name: "Pause" });
  await pause.click();
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
  await expect(page.locator("[data-day-clock]")).toContainText("Paused");
  const before = await page.evaluate(() => ({ tick: window.gameTest.tickCount(), guest: window.gameTest.guests()[0] }));
  await page.waitForTimeout(1_000);
  const after = await page.evaluate(() => ({ tick: window.gameTest.tickCount(), guest: window.gameTest.guests()[0] }));
  expect(after.tick).toBe(before.tick);
  expect({ x: after.guest.x, y: after.guest.y, height: after.guest.height }).toEqual({ x: before.guest.x, y: before.guest.y, height: before.guest.height });

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await openTool(page, "Paths");
  await page.getByRole("button", { name: "Build paths" }).click();
  const pathPoints = await page.evaluate(() => [
    window.gameTest.project({ x: 48.5, y: 51.5, z: 2 }),
    window.gameTest.project({ x: 48.5, y: 47.5, z: 2 }),
  ]);
  await moveOverCanvas(page, pathPoints[0]);
  const pathStart = await canvasToPage(page, pathPoints[0]);
  await page.mouse.move(pathStart.x, pathStart.y);
  await page.mouse.down();
  const pathEnd = await canvasToPage(page, pathPoints[1]);
  await page.mouse.move(pathEnd.x, pathEnd.y, { steps: 8 });
  await page.mouse.up();
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  expect(await page.evaluate(() => window.gameTest.paths().length)).toBeGreaterThan(0);

  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Watch", exact: true }).click();
  await page.locator("canvas").focus();
  await page.keyboard.press("Space");
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "false");
  await expect.poll(() => page.evaluate(() => window.gameTest.tickCount()), { timeout: 5_000 }).toBeGreaterThan(before.tick);
});
