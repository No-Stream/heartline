import { expect, test, type Page } from "@playwright/test";
import type { Facility } from "../src/facilities";
import type { GameTest } from "../src/game-test";
import { browserLaunchOptions } from "./fixtures/browser-launch-options";
import { canvasSize, clickCanvas, moveOverCanvas } from "./game-page";

test.use({
  deviceScaleFactor: 1,
  launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]),
});

for (const rotation of [0, 2] as const) {
  test(`demo park screenshot at rotation ${rotation}${rotation === 0 ? " @fast" : ""}`, async ({ page }) => {
    await page.goto("/?test=1");
    await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
    const flatRideStates = await page.evaluate(() => window.gameTest.facilities()
      .filter(({ kind }) => kind === "carousel" || kind === "ferris_wheel")
      .map(({ kind, ride_state, ride_progress }) => ({ kind, ride_state, ride_progress })));
    expect(flatRideStates).toEqual([
      { kind: "carousel", ride_state: "idle", ride_progress: 0 },
      { kind: "ferris_wheel", ride_state: "idle", ride_progress: 0 },
    ]);
    await page.evaluate((viewRotation) => {
      const game: GameTest = window.gameTest;
      game.setCamera({ centerX: 228, centerY: 252, zoom: 0.5, rotation: viewRotation });
    }, rotation);
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await expect(page.locator("canvas")).toHaveScreenshot(`demo-park-rotation-${rotation}.png`, {
      animations: "disabled",
      threshold: 0.1,
      maxDiffPixelRatio: 0.001,
    });
  });
}

async function centerOnFacility(page: Page, facility: Facility): Promise<void> {
  await page.evaluate((building) => {
    const game: GameTest = window.gameTest;
    game.setCamera({ centerX: building.x + 1, centerY: building.y + 1, zoom: 2, rotation: 0 });
  }, facility);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
}

async function findFacilityHit(page: Page, facility: Facility): Promise<{ x: number; y: number } | null> {
  const size = await canvasSize(page);
  return page.evaluate(({ facility: { id, x, y, z }, size }) => {
    const anchor = window.gameTest.project({ x: x + 1, y: y + 1, z: z / 4 });
    const minX = Math.max(0, Math.floor(anchor.x - 180));
    const maxX = Math.min(size.width - 1, Math.ceil(anchor.x + 180));
    const minY = Math.max(0, Math.floor(anchor.y - 180));
    const maxY = Math.min(size.height - 1, Math.ceil(anchor.y + 100));
    for (let screenY = minY; screenY <= maxY; screenY += 6) {
      for (let screenX = minX; screenX <= maxX; screenX += 6) {
        const selection = window.gameTest.pick(screenX, screenY);
        if (selection && "id" in selection && selection.id === `facility-${id}`) return { x: screenX, y: screenY };
      }
    }
    return null;
  }, { facility, size });
}

