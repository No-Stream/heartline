import { expect, test, type Page } from "@playwright/test";

test.use({ actionTimeout: 10_000, viewport: { width: 1100, height: 1200 } });
test("guests ride a player-built coaster and can be selected", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 241, centerY: 252, zoom: 1.5, rotation: 0 }));
  await page.getByLabel("Station height above ground (metres)").fill("4");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  await clickWorld(page, 241.5, 249.5, 2);
  await clickWorld(page, 241.5, 251.5, 2);
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  const pieces = ["LiftLevelToGentleUp", "Lift", "Lift", "Lift", "Lift", "Lift", "LiftGentleUpToLevel", "LiftFlat", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit", "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel", "Loop", "TurnLargeEntry", "TurnLargeExit"];
  for (const [index, piece] of pieces.entries()) {
    await choices.locator(`[data-piece="${piece}"]`).click();
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.getByRole("button", { name: "Open coaster" })).toBeEnabled();

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 241, centerY: 253, zoom: 2, rotation: 0 }));
  await page.getByRole("button", { name: "Flatten", exact: true }).click();
  await page.getByLabel("Brush size").selectOption("1");
  for (const [y, height] of [[255, 8], [254, 9], [253, 10], [252, 11], [251, 12], [250, 12]] as const) {
    await page.getByLabel("Flatten height").fill(String(height));
    for (const x of [238, 244]) {
      await page.getByRole("button", { name: "Apply tool" }).click();
      await clickTerrain(page, x, y);
      await expect(page.locator(".landscaping__status"), `flatten tile (${x}, ${y}) to height ${height}`).toHaveText("Terrain updated");
    }
  }
  await page.getByLabel("Flatten height").fill("12");
  for (const x of [239, 243]) {
    await page.getByRole("button", { name: "Apply tool" }).click();
    await clickTerrain(page, x, 250);
    await expect(page.locator(".landscaping__status"), `flatten station access tile (${x}, 250)`).toHaveText("Terrain updated");
  }
  await page.getByRole("button", { name: "Cancel tool" }).click();
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 241, centerY: 253, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Path type").selectOption("queue");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragWorld(page, { x: 243.5, y: 250.5, z: 3 }, { x: 244.5, y: 256.5, z: 2 });
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  await page.getByRole("region", { name: "Paths" }).getByRole("button", { name: "Cancel" }).click();
  await page.getByLabel("Path type").selectOption("normal");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragWorld(page, { x: 239.5, y: 250.5, z: 3 }, { x: 238.5, y: 256.5, z: 2 });
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  await page.getByRole("region", { name: "Paths" }).getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: "Open coaster" }).click();
  await expect(page.getByRole("button", { name: "Close coaster" })).toBeVisible();
  let riderId = "";
  const queuedIds = new Set<string>();
  for (let ticks = 0; ticks < 30_000 && !riderId; ticks += 100) {
    await page.evaluate(() => window.gameTest.advanceTicks(100));
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    const queued = await page.evaluate(() => {
      const queue = new Set(window.gameTest.paths().filter((path) => path.kind === 1).map((path) => `${path.x},${path.y}`));
      return window.gameTest.guests().filter((guest) => queue.has(`${Math.floor(guest.x)},${Math.floor(guest.y)}`)).map((guest) => guest.id);
    });
    for (const id of queued) queuedIds.add(id);
    const riders = await page.evaluate(() => window.gameTest.riders().map((rider) => rider.id));
    riderId = riders.find((id) => queuedIds.has(id)) ?? "";
  }
  expect(queuedIds.size, "a guest should enter the queue").toBeGreaterThan(0);
  expect(riderId, "a queued guest should board the coaster").not.toBe("");
  await page.evaluate((id) => {
    const rider = window.gameTest.riders().find((entry) => entry.id === id);
    if (!rider) throw new Error("Rider disappeared before selection");
    window.gameTest.setCamera({ centerX: rider.x, centerY: rider.y, zoom: 2, rotation: 0 });
  }, riderId);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const pick = await page.evaluate((id) => {
    for (let y = 0; y < 500; y += 2) for (let x = 0; x < 800; x += 2) {
      const matches = [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]].every(([dx, dy]) => {
        const selected = window.gameTest.pick(x + dx, y + dy);
        return selected && "id" in selected && selected.id === id;
      });
      if (matches) return { x, y };
    }
    return null;
  }, riderId);
  if (!pick) throw new Error(`Rider ${riderId} was not visible and pickable`);
  await page.locator("canvas").scrollIntoViewIfNeeded();
  const bounds = await page.locator("canvas").boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.mouse.click(bounds.x + pick.x * bounds.width / 800, bounds.y + pick.y * bounds.height / 500);
  await expect(page.locator("canvas")).toHaveAttribute("data-selection", new RegExp(`"id":"${riderId}"`));
  await expect(page.getByTestId("guest-panel")).toHaveAttribute("data-guest-id", riderId);
  await expect(page.locator("canvas")).toHaveScreenshot("coaster-boarding.png", { animations: "disabled" });
  let exited = false;
  for (let ticks = 0; ticks < 100_000 && !exited; ticks += 100) {
    await page.evaluate(() => window.gameTest.advanceTicks(100));
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    exited = await page.evaluate((id) => !window.gameTest.riders().some((rider) => rider.id === id) && window.gameTest.guests().some((guest) => guest.id === id), riderId);
  }
  expect(exited).toBe(true);
  await page.evaluate((id) => window.gameTest.requestGuestDetail(id), riderId);
  await expect.poll(() => page.evaluate((id) => window.gameTest.guestDetail()?.id === id, riderId)).toBe(true);
  expect(await page.evaluate(() => window.gameTest.guestDetail()?.memories.some((memory) => memory.kind === "ride"))).toBe(true);
});

async function clickWorld(page: Page, x: number, y: number, z: number): Promise<void> {
  const point = await page.evaluate((position) => window.gameTest.project(position), { x, y, z });
  await page.locator("canvas").scrollIntoViewIfNeeded();
  const bounds = await page.locator("canvas").boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.mouse.click(bounds.x + point.x * bounds.width / 800, bounds.y + point.y * bounds.height / 500);
}

async function clickTerrain(page: Page, x: number, y: number): Promise<void> {
  let point: { x: number; y: number } | null = null;
  for (const rotation of [0, 1, 2, 3] as const) {
    await page.evaluate(({ x, y, rotation }) => window.gameTest.setCamera({ centerX: x + 0.5, centerY: y + 0.5, zoom: 2, rotation }), { x, y, rotation });
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    point = await page.evaluate(({ x, y }) => {
    const projected = window.gameTest.project({ x: x + 0.5, y: y + 0.5, z: 2 });
    const candidates: { x: number; y: number; distance: number }[] = [];
    for (let dy = -48; dy <= 8; dy += 2) for (let dx = -32; dx <= 32; dx += 2) {
      const screenX = projected.x + dx, screenY = projected.y + dy;
      if (screenX < 2 || screenX >= 798 || screenY < 2 || screenY >= 498) continue;
      const matches = [[0, 0], [-2, 0], [2, 0], [0, -2], [0, 2]].every(([ox, oy]) => {
        const pick = window.gameTest.pickSurface(screenX + ox, screenY + oy);
        return pick?.kind === "terrain" && pick.x === x && pick.y === y;
      });
      if (matches) candidates.push({ x: screenX, y: screenY, distance: dx * dx + dy * dy });
    }
    candidates.sort((first, second) => first.distance - second.distance);
      return candidates[0] ?? null;
    }, { x, y });
    if (point) break;
  }
  if (!point) throw new Error(`Terrain tile ${x},${y} is not visible in any rotation`);
  await page.locator("canvas").scrollIntoViewIfNeeded();
  const bounds = await page.locator("canvas").boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.locator("canvas").evaluate((canvas) => {
    canvas.addEventListener("pointerup", (event) => {
      const pointer = event as PointerEvent;
      const rect = canvas.getBoundingClientRect();
      const screenX = (pointer.clientX - rect.left) * 800 / rect.width;
      const screenY = (pointer.clientY - rect.top) * 500 / rect.height;
      canvas.dataset.lastTerrainPick = JSON.stringify(window.gameTest.pickSurface(screenX, screenY));
    }, { capture: true, once: true });
  });
  await page.mouse.click(bounds.x + point.x * bounds.width / 800, bounds.y + point.y * bounds.height / 500);
  await expect(page.locator("canvas")).toHaveAttribute("data-last-terrain-pick", new RegExp(`"x":${x},"y":${y}(?:,|})`));
}

async function dragWorld(page: Page, start: { x: number; y: number; z: number }, end: { x: number; y: number; z: number }): Promise<void> {
  const points = await page.evaluate(({ start, end }) => ({ start: window.gameTest.project(start), end: window.gameTest.project(end) }), { start, end });
  await page.locator("canvas").scrollIntoViewIfNeeded();
  const bounds = await page.locator("canvas").boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  const screen = (point: { x: number; y: number }) => ({ x: bounds.x + point.x * bounds.width / 800, y: bounds.y + point.y * bounds.height / 500 });
  const from = screen(points.start), to = screen(points.end);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
}
