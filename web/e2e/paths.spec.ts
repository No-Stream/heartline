import { expect, test, type Page } from "@playwright/test";

test.use({
  viewport: { width: 1100, height: 1200 },
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
});

test("rebuilds the entrance path through the UI and guests walk to the food stall", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);

  const foodStall = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "food_stall") ?? null);
  if (!foodStall) throw new Error("The demo park has no food stall");
  const guestIds = await page.evaluate(() => window.gameTest.guests().map(({ id }) => id));
  expect(guestIds.length).toBeGreaterThan(0);
  const previousFoodTicks = new Map<string, number>();
  for (const guestId of guestIds) {
    const detail = await guestDetail(page, guestId);
    previousFoodTicks.set(guestId, Math.max(0, ...detail.memories
      .filter((memory) => memory.kind === "food" && memory.placeId === foodStall.id)
      .map((memory) => memory.tick)));
  }

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 218, centerY: 253, zoom: 2, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByRole("button", { name: "Remove paths" }).click();
  await clickTile(page, 218, 252, 2);
  await expect.poll(() => pathAt(page, 218, 252, 2)).toBe(false);

  await page.getByLabel("Path type").selectOption("normal");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragTiles(page, { x: 218, y: 250, z: 2 }, { x: 218, y: 256, z: 2 });
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect.poll(() => pathAt(page, 218, 252, 2)).toBe(true);

  const corridorTilesByGuest = new Map<string, Set<number>>();
  let walkedRebuiltCorridor = false;
  let newlyVisitedFoodStall = false;
  for (let advancedTicks = 0; advancedTicks < 100_000; advancedTicks += 250) {
    await page.evaluate((count) => window.gameTest.advanceTicks(count), 250);
    const guestPositions = await page.evaluate(() => window.gameTest.guests().map(({ id, x, y }) => ({
      id,
      tileX: Math.floor(x),
      tileY: Math.floor(y),
    })));
    for (const guest of guestPositions) {
      if (guest.tileX !== 218 || guest.tileY < 250 || guest.tileY > 256) continue;
      const visitedTiles = corridorTilesByGuest.get(guest.id) ?? new Set<number>();
      visitedTiles.add(guest.tileY);
      corridorTilesByGuest.set(guest.id, visitedTiles);
      if (visitedTiles.size > 1) walkedRebuiltCorridor = true;
    }

    if (advancedTicks % 5_000 === 4_750) {
      for (const guestId of guestIds) {
        const detail = await guestDetail(page, guestId);
        const previousTick = previousFoodTicks.get(guestId) ?? 0;
        if (detail.memories.some((memory) => memory.kind === "food" && memory.placeId === foodStall.id && memory.tick > previousTick)) {
          newlyVisitedFoodStall = true;
        }
      }
    }
    if (walkedRebuiltCorridor && newlyVisitedFoodStall) return;
  }
  expect(walkedRebuiltCorridor, "a guest should walk across multiple tiles of the rebuilt corridor").toBe(true);
  expect(newlyVisitedFoodStall, "a guest should visit the food stall after the path rebuild").toBe(true);
});

async function guestDetail(page: Page, guestId: string) {
  await page.evaluate((id) => window.gameTest.requestGuestDetail(id), guestId);
  await expect.poll(() => page.evaluate((id) => window.gameTest.guestDetail()?.id === id, guestId), { timeout: 5_000 }).toBe(true);
  const detail = await page.evaluate(() => window.gameTest.guestDetail());
  if (!detail) throw new Error(`Guest detail is missing for ${guestId}`);
  return detail;
}

test("builds sloped paths and a queue through the UI", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 251, centerY: 255.5, zoom: 0.5, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  await page.getByLabel("Path type").selectOption("normal");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragTiles(page, { x: 242, y: 256, z: 2 }, { x: 260, y: 256, z: 5.75 });
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByLabel("Path type").selectOption("queue");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragTiles(page, { x: 242, y: 255, z: 2 }, { x: 260, y: 255, z: 5.75 });
  await page.getByRole("button", { name: "Cancel" }).click();

  await expect.poll(() => page.evaluate(() => {
    const point = window.gameTest.project({ x: 250.5, y: 255.5, z: 3.375 });
    const selected = window.gameTest.pick(point.x, point.y);
    return selected?.kind === "path" && selected.x === 250 && selected.y === 255 && Math.abs(selected.height - 13.5) < 1;
  })).toBe(true);
  await expect(page.getByLabel("Path type")).toHaveValue("queue");
  await expect(page.locator("main")).toHaveScreenshot("path-network-slopes-queue.png", {
    animations: "disabled",
    threshold: 0.1,
    maxDiffPixelRatio: 0.001,
  });
});

async function pathAt(page: Page, x: number, y: number, z: number): Promise<boolean> {
  return page.evaluate(({ tileX, tileY, height }) => {
    const point = window.gameTest.project({ x: tileX + 0.5, y: tileY + 0.5, z: height });
    return window.gameTest.pick(point.x, point.y)?.kind === "path";
  }, { tileX: x, tileY: y, height: z });
}

async function clickTile(page: Page, x: number, y: number, z: number): Promise<void> {
  const point = await page.evaluate(({ tileX, tileY, height }) => window.gameTest.project({ x: tileX + 0.5, y: tileY + 0.5, z: height }), {
    tileX: x,
    tileY: y,
    height: z,
  });
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.mouse.click(bounds.x + point.x * bounds.width / 800, bounds.y + point.y * bounds.height / 500);
}

async function dragTiles(page: Page, start: { x: number; y: number; z: number }, end: { x: number; y: number; z: number }): Promise<void> {
  const points = await page.evaluate(({ startTile, endTile }) => ({
    start: window.gameTest.project({ x: startTile.x + 0.5, y: startTile.y + 0.5, z: startTile.z }),
    end: window.gameTest.project({ x: endTile.x + 0.5, y: endTile.y + 0.5, z: endTile.z }),
  }), { startTile: start, endTile: end });
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  const screenPoint = (point: { x: number; y: number }): { x: number; y: number } => ({
    x: bounds.x + point.x * bounds.width / 800,
    y: bounds.y + point.y * bounds.height / 500,
  });
  const startPoint = screenPoint(points.start);
  const endPoint = screenPoint(points.end);
  await page.mouse.move(startPoint.x, startPoint.y);
  await page.mouse.down();
  await page.mouse.move(endPoint.x, endPoint.y, { steps: 12 });
  await page.mouse.up();
}
