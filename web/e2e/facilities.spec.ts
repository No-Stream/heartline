import { expect, test, type Page } from "@playwright/test";
import type { Facility } from "../src/facilities";
import type { GameTest } from "../src/game-test";
import { canvasSize, clickCanvas, moveOverCanvas, openTool } from "./game-page";

test.use({
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
});

test("places a food stall through the UI and guests visit it", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await expect.poll(() => page.evaluate(() => window.gameTest.facilities().length)).toBe(7);
  await openTool(page, "Facilities");

  const demoFoodStall = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "food_stall") ?? null);
  if (!demoFoodStall) throw new Error("The demo park has no food stall to replace");
  await centerOnFacility(page, demoFoodStall);
  await page.getByRole("button", { name: "Remove facility" }).click();
  const demoFoodStallHit = await findFacilityHit(page, demoFoodStall);
  if (!demoFoodStallHit) throw new Error("Could not find the demo food stall in the rendered park");
  await clickCanvas(page, demoFoodStallHit);
  await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().some((facility) => facility.id === id), demoFoodStall.id)).toBe(false);

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 240, centerY: 255, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const facilitiesPanel = page.getByRole("region", { name: "Park facilities" });
  await facilitiesPanel.getByRole("radio", { name: "Food stall" }).click();
  await expect(facilitiesPanel.getByRole("radio", { name: "Food stall" })).toHaveAttribute("aria-checked", "true");
  await page.getByRole("button", { name: "Place facility" }).click();
  const targetTile = await page.evaluate(() => {
    const point = window.gameTest.project({ x: 240.5, y: 254.5, z: 2 });
    return { point, selection: window.gameTest.pick(point.x, point.y) };
  });
  expect(targetTile.selection).toMatchObject({ kind: "terrain", x: 240, y: 254, height: 8 });
  await moveOverCanvas(page, targetTile.point);
  await expect(facilitiesPanel.getByRole("group", { name: "Facility direction" })).toContainText("South");
  await clickCanvas(page, targetTile.point);
  await expect(facilitiesPanel.getByRole("status")).toHaveText("Facility placed.");

  const placedFoodStall = await page.evaluate(() => window.gameTest.facilities().find(({ x, y, kind }) => kind === "food_stall" && x === 240 && y === 254) ?? null);
  expect(placedFoodStall).toMatchObject({ kind: "food_stall", x: 240, y: 254, z: 8, rotation: 2 });
  if (!placedFoodStall) throw new Error("The newly placed food stall is missing from the simulation");

  const guestIds = await page.evaluate(() => window.gameTest.guests().map(({ id }) => id));
  expect(guestIds.length).toBeGreaterThan(0);
  let guestVisitedFoodStall = false;
  for (let advancedTicks = 0; advancedTicks < 100_000; advancedTicks += 5_000) {
    await page.evaluate((count) => window.gameTest.advanceTicks(count), 5_000);
    for (const guestId of guestIds) {
      await page.evaluate((id) => window.gameTest.requestGuestDetail(id), guestId);
      await expect.poll(() => page.evaluate((id) => window.gameTest.guestDetail()?.id === id, guestId), { timeout: 5_000 }).toBe(true);
      const detail = await page.evaluate(() => window.gameTest.guestDetail());
      if (detail?.memories.some((memory) => memory.kind === "food" && memory.placeId === placedFoodStall.id)) {
        guestVisitedFoodStall = true;
        break;
      }
    }
    if (guestVisitedFoodStall) break;
  }
  expect(guestVisitedFoodStall, "A guest should visit the food stall placed next to the path").toBe(true);

  let currentFacility: Facility = placedFoodStall;
  for (const [kind, label] of [["drink_stall", "Drink stall"], ["restroom", "Restroom"], ["souvenir", "Souvenir shop"]] as const) {
    await page.getByRole("button", { name: "Remove facility" }).click();
    const facilityHit = await findFacilityHit(page, currentFacility);
    if (!facilityHit) throw new Error(`Could not find ${currentFacility.kind} in the rendered park`);
    await clickCanvas(page, facilityHit);
    await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().some(({ id: currentId }) => currentId === id), currentFacility.id)).toBe(false);

    await facilitiesPanel.getByRole("radio", { name: label }).click();
    await page.getByRole("button", { name: "Place facility" }).click();
    await moveOverCanvas(page, targetTile.point);
    await expect(facilitiesPanel.getByRole("group", { name: "Facility direction" })).toContainText("South");
    await clickCanvas(page, targetTile.point);
    await expect(facilitiesPanel.getByRole("status")).toHaveText("Facility placed.");
    const nextFacility = await page.evaluate(({ kind: selectedKind }) => window.gameTest.facilities().find(({ kind, x, y }) => kind === selectedKind && x === 240 && y === 254) ?? null, { kind });
    expect(nextFacility).toMatchObject({ kind, x: 240, y: 254, z: 8, rotation: 2 });
    if (!nextFacility) throw new Error(`The newly placed ${kind} is missing from the simulation`);
    currentFacility = nextFacility;
  }

  for (const [kind, label] of [["entrance", "Entrance"], ["carousel", "Carousel"], ["ferris_wheel", "Ferris wheel"]] as const) {
    const originalFacility = await page.evaluate((selectedKind) => window.gameTest.facilities().find(({ kind }) => kind === selectedKind) ?? null, kind);
    if (!originalFacility) throw new Error(`The demo park has no ${kind} to replace`);
    await centerOnFacility(page, originalFacility);
    await page.getByRole("button", { name: "Remove facility" }).click();
    const originalHit = await findFacilityHit(page, originalFacility);
    if (!originalHit) throw new Error(`Could not find the demo ${kind} in the rendered park`);
    await clickCanvas(page, originalHit);
    await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().some(({ id: currentId }) => currentId === id), originalFacility.id)).toBe(false);

    await facilitiesPanel.getByRole("radio", { name: label }).click();
    for (let turn = 0; turn < originalFacility.rotation; turn += 1) {
      await facilitiesPanel.getByRole("button", { name: "Turn clockwise" }).click();
    }
    await page.getByRole("button", { name: "Place facility" }).click();
    const target = await page.evaluate((facility) => {
      const point = window.gameTest.project({ x: facility.x + 0.5, y: facility.y + 0.5, z: facility.z / 4 });
      return { point, selection: window.gameTest.pick(point.x, point.y) };
    }, originalFacility);
    expect(target.selection).toMatchObject({ kind: "terrain", x: originalFacility.x, y: originalFacility.y });
    await moveOverCanvas(page, target.point);
    await expect(facilitiesPanel.getByRole("group", { name: "Facility direction" })).toContainText(["North", "East", "South", "West"][originalFacility.rotation]!);
    await clickCanvas(page, target.point);
    await expect(facilitiesPanel.getByRole("status")).toHaveText("Facility placed.");
    await expect.poll(() => page.evaluate(({ kind: selectedKind, x, y }) => window.gameTest.facilities().some((facility) => facility.kind === selectedKind && facility.x === x && facility.y === y), originalFacility)).toBe(true);
  }

  await page.getByRole("button", { name: "Remove facility" }).click();
  const souvenirHit = await findFacilityHit(page, currentFacility);
  if (!souvenirHit) throw new Error("Could not find the souvenir shop to replace");
  await clickCanvas(page, souvenirHit);
  await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().some(({ id: currentId }) => currentId === id), currentFacility.id)).toBe(false);

  for (const { x, y, direction, rotation } of [
    { x: 241, y: 257, direction: "North", rotation: 0 },
    { x: 233, y: 259, direction: "East", rotation: 1 },
    { x: 236, y: 259, direction: "West", rotation: 3 },
  ] as const) {
    await page.evaluate(({ centerX, centerY }) => window.gameTest.setCamera({ centerX, centerY, zoom: 1, rotation: 0 }), { centerX: x + 1, centerY: y + 1 });
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    await facilitiesPanel.getByRole("radio", { name: "Food stall" }).click();
    await page.getByRole("button", { name: "Place facility" }).click();
    const candidate = await page.evaluate(({ x: tileX, y: tileY }) => {
      const point = window.gameTest.project({ x: tileX + 0.5, y: tileY + 0.5, z: 2 });
      return { point, selection: window.gameTest.pick(point.x, point.y) };
    }, { x, y });
    expect(candidate.selection).toMatchObject({ kind: "terrain", x, y, height: 8 });
    await moveOverCanvas(page, candidate.point);
    await expect(facilitiesPanel.getByRole("group", { name: "Facility direction" })).toContainText(direction);
    await clickCanvas(page, candidate.point);
    await expect(facilitiesPanel.getByRole("status")).toHaveText("Facility placed.");
    const nextFoodStall = await page.evaluate(({ x: tileX, y: tileY }) => window.gameTest.facilities().find(({ kind, x, y }) => kind === "food_stall" && x === tileX && y === tileY) ?? null, { x, y });
    expect(nextFoodStall).toMatchObject({ x, y, z: 8, rotation });
    if (!nextFoodStall) throw new Error(`The food stall facing ${direction.toLowerCase()} is missing`);
    currentFacility = nextFoodStall;
    await page.getByRole("button", { name: "Remove facility" }).click();
    const nextFoodStallHit = await findFacilityHit(page, currentFacility);
    if (!nextFoodStallHit) throw new Error(`Could not find the food stall facing ${direction.toLowerCase()}`);
    await clickCanvas(page, nextFoodStallHit);
    await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().some(({ id: currentId }) => currentId === id), currentFacility.id)).toBe(false);
  }
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

