import { expect, test, type Page } from "@playwright/test";
import type { Facility } from "../src/facilities";
import type { GameTest } from "../src/game-test";

test.use({
  viewport: { width: 1100, height: 1200 },
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
});

test("places a food stall through the UI and guests visit it", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await expect.poll(() => page.evaluate(() => window.gameTest.facilities().length)).toBe(7);

  const demoFoodStall = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "food_stall") ?? null);
  if (!demoFoodStall) throw new Error("The demo park has no food stall to replace");
  await centerOnFacility(page, demoFoodStall);
  await page.getByRole("button", { name: "Remove facility" }).click();
  const demoFoodStallHit = await findFacilityHit(page, demoFoodStall);
  if (!demoFoodStallHit) throw new Error("Could not find the demo food stall in the rendered park");
  await clickCanvasPoint(page, demoFoodStallHit);
  await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().some((facility) => facility.id === id), demoFoodStall.id)).toBe(false);

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 240, centerY: 255, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Facility").selectOption("food_stall");
  await page.getByLabel("Direction").selectOption("2");
  await page.getByRole("button", { name: "Place facility" }).click();
  const targetTile = await page.evaluate(() => {
    const point = window.gameTest.project({ x: 240.5, y: 254.5, z: 2 });
    return { point, selection: window.gameTest.pick(point.x, point.y) };
  });
  expect(targetTile.selection).toMatchObject({ kind: "terrain", x: 240, y: 254, height: 8 });
  await clickCanvasPoint(page, targetTile.point);
  await expect(page.locator("[data-facilities] p[role='status']")).toHaveText("Facility placed.");

  const placedFoodStall = await page.evaluate(() => window.gameTest.facilities().find(({ x, y, kind }) => kind === "food_stall" && x === 240 && y === 254) ?? null);
  expect(placedFoodStall).toMatchObject({ kind: "food_stall", x: 240, y: 254, z: 8, rotation: 2 });
  if (!placedFoodStall) throw new Error("The newly placed food stall is missing from the simulation");

  const guestIds = await page.evaluate(() => window.gameTest.guests().map(({ id }) => id));
  expect(guestIds.length).toBeGreaterThan(0);
  for (let advancedTicks = 0; advancedTicks < 100_000; advancedTicks += 5_000) {
    await page.evaluate((count) => window.gameTest.advanceTicks(count), 5_000);
    for (const guestId of guestIds) {
      await page.evaluate((id) => window.gameTest.requestGuestDetail(id), guestId);
      await expect.poll(() => page.evaluate((id) => window.gameTest.guestDetail()?.id === id, guestId), { timeout: 5_000 }).toBe(true);
      const detail = await page.evaluate(() => window.gameTest.guestDetail());
      if (detail?.memories.some((memory) => memory.kind === "food" && memory.placeId === placedFoodStall.id)) return;
    }
  }
  throw new Error("No guest recorded a visit to the food stall placed in the demo park");
});

for (const rotation of [0, 2] as const) {
  test(`demo park screenshot at rotation ${rotation}`, async ({ page }) => {
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
  return page.evaluate(({ id, x, y, z }) => {
    const anchor = window.gameTest.project({ x: x + 1, y: y + 1, z: z / 4 });
    const minX = Math.max(0, Math.floor(anchor.x - 180));
    const maxX = Math.min(799, Math.ceil(anchor.x + 180));
    const minY = Math.max(0, Math.floor(anchor.y - 180));
    const maxY = Math.min(499, Math.ceil(anchor.y + 100));
    for (let screenY = minY; screenY <= maxY; screenY += 6) {
      for (let screenX = minX; screenX <= maxX; screenX += 6) {
        const selection = window.gameTest.pick(screenX, screenY);
        if (selection && "id" in selection && selection.id === `facility-${id}`) return { x: screenX, y: screenY };
      }
    }
    return null;
  }, facility);
}

async function clickCanvasPoint(page: Page, point: { x: number; y: number }): Promise<void> {
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.mouse.click(bounds.x + point.x * bounds.width / 800, bounds.y + point.y * bounds.height / 500);
}
