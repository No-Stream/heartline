import { expect, test, type Page } from "@playwright/test";
import type { Camera } from "../src/camera";
import { canvasToPage, clickCanvas, clickTile, openTool, selectUnits, showBuilderTab } from "./game-page";

test.use({ actionTimeout: 10_000 });
test("guests ride a player-built coaster and can be selected @slow", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 241, centerY: 252, zoom: 1.5, rotation: 0 }));
  await page.getByLabel("Station height above ground (metres)").fill("4");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  await clickTile(page, { x: 241, y: 249, z: 2 });
  await clickWorld(page, 241.5, 251.5, 2);
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  const pieces = ["LevelToGentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUpToLevel", "Straight", "OneTileStraight", "OneTileStraight", "OffsetLeft", "TurnLargeEntry", "TurnLargeExit", "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel", "Loop", "OffsetRight", "TurnLargeEntry", "TurnLargeExit", "OneTileStraight"];
  for (const [index, piece] of pieces.entries()) {
    if (index === 0 || index === 8) {
      const chainLift = page.getByRole("button", { name: "Chain lift" });
      await chainLift.click();
      await expect(chainLift).toHaveAttribute("aria-pressed", index === 0 ? "true" : "false");
    }
    await choices.locator(`[data-piece="${piece}"]`).click();
    if (piece === "Loop") {
      await page.getByRole("group", { name: "Element size" }).locator('[data-speed-match-variant="Loop"]').click();
    }
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
  }
  await expect(page.getByRole("button", { name: "Finish coaster" })).toBeEnabled();
  await page.getByRole("button", { name: "Finish coaster" }).click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-finish-preview", "true");
  await page.getByRole("button", { name: "Confirm coaster finish" }).click();
  await expect(page.getByRole("button", { name: "Open coaster" })).toBeEnabled();

  await openTool(page, "Landscape");
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
  await openTool(page, "Paths");
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
  await openTool(page, "Facilities");
  const entrance = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "entrance") ?? null);
  if (!entrance) throw new Error("The park entrance is missing");
  const entrancePrice = page.getByRole("spinbutton", { name: `Price of entrance ${entrance.id}` });
  await entrancePrice.fill("8");
  await entrancePrice.press("Tab");
  await expect.poll(() => page.evaluate((id) => window.gameTest.facilities().find(({ id: currentId }) => currentId === id)?.price, entrance.id)).toBe(8);
  await openTool(page, "Coaster");
  await showBuilderTab(page, "Operate");
  const coasterPrice = page.getByRole("spinbutton", { name: "Coaster ticket price" });
  await coasterPrice.fill("9");
  await coasterPrice.press("Tab");
  await expect(coasterPrice).toHaveValue("9");
  await showBuilderTab(page, "Build");
  await page.getByRole("button", { name: "Open coaster" }).click();
  await expect(page.getByRole("button", { name: "Close coaster" })).toBeVisible();
  let riderId = "";
  const queuedIds = new Set<string>();
  const queueCamera: Camera = { centerX: 241, centerY: 253, zoom: 1, rotation: 0 };
  for (let ticks = 0; ticks < 30_000 && !riderId; ticks += 100) {
    await advanceTicksAndRender(page, 100, queueCamera);
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
  await expect.poll(async () => Number(await page.locator("[data-coaster-income]").textContent())).toBeGreaterThan(0);
  const riderCamera = await page.evaluate((id): Camera => {
    const rider = window.gameTest.riders().find((entry) => entry.id === id);
    if (!rider) throw new Error("Rider disappeared before selection");
    return { centerX: rider.x - rider.z, centerY: rider.y - rider.z, zoom: 2, rotation: 0 };
  }, riderId);
  await page.evaluate((camera) => window.gameTest.setCamera(camera), riderCamera);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const pick = await page.evaluate((id) => {
    const canvas = document.querySelector("canvas")!;
    const width = canvas.width;
    const height = canvas.height;
    // Keep the full-canvas search, but batch the synchronous WebGL readback.
    const picks = window.gameTest.pickRegion(0, 0, width, height);
    const neighbors = [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]] as const;
    const pickAt = (x: number, y: number) => {
      if (x < 0 || y < 0 || x >= width || y >= height) return null;
      return picks[y * width + x] ?? null;
    };
    const hasId = (selected: ReturnType<typeof window.gameTest.pick>): boolean =>
      selected !== null && "id" in selected && selected.id === id;
    for (let y = 0; y < height; y += 2) for (let x = 0; x < width; x += 2) {
      const matches = neighbors.every(([dx, dy]) => hasId(pickAt(x + dx, y + dy)));
      if (matches) {
        // Check that the batch result agrees with the exact point API at the returned visible target.
        const directMatches = neighbors.every(([dx, dy]) => hasId(window.gameTest.pick(x + dx, y + dy)));
        if (!directMatches) throw new Error("Batched and point picking disagree for the visible rider");
        return { x, y };
      }
    }
    return null;
  }, riderId);
  if (!pick) throw new Error(`Rider ${riderId} was not visible and pickable`);
  await page.locator("canvas").scrollIntoViewIfNeeded();
  await expect(page.locator("canvas")).toHaveScreenshot("coaster-boarding.png", { animations: "disabled" });
  await clickCanvas(page, pick);
  await expect(page.locator("canvas")).toHaveAttribute("data-selection", new RegExp(`"id":"${riderId}"`));
  await expect(page.getByTestId("guest-panel")).toHaveAttribute("data-guest-id", riderId);
  let exited = false;
  for (let ticks = 0; ticks < 100_000 && !exited; ticks += 100) {
    await advanceTicksAndRender(page, 100, riderCamera);
    exited = await page.evaluate((id) => !window.gameTest.riders().some((rider) => rider.id === id) && window.gameTest.guests().some((guest) => guest.id === id), riderId);
  }
  expect(exited).toBe(true);
  await page.evaluate((id) => window.gameTest.requestGuestDetail(id), riderId);
  await expect.poll(() => page.evaluate((id) => {
    const detail = window.gameTest.guestDetail();
    return detail?.id === id && detail.memories.some((memory) => memory.kind === "ride" && memory.rideId === 0xffff_ffff);
  }, riderId)).toBe(true);
});

async function advanceTicksAndRender(page: Page, count: number, camera: Camera): Promise<void> {
  await page.evaluate(async ({ count, camera }) => {
    await window.gameTest.advanceTicks(count);
    window.gameTest.setCamera(camera);
  }, { count, camera });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
}

async function clickWorld(page: Page, x: number, y: number, z: number): Promise<void> {
  const point = await page.evaluate((position) => window.gameTest.project(position), { x, y, z });
  await clickCanvas(page, point);
}

async function clickTerrain(page: Page, x: number, y: number): Promise<void> {
  let point: { x: number; y: number } | null = null;
  for (const rotation of [0, 1, 2, 3] as const) {
    await page.evaluate(({ x, y, rotation }) => window.gameTest.setCamera({ centerX: x + 0.5, centerY: y + 0.5, zoom: 2, rotation }), { x, y, rotation });
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    point = await page.evaluate(({ x, y }) => {
    const projected = window.gameTest.project({ x: x + 0.5, y: y + 0.5, z: 2 });
    const canvas = document.querySelector("canvas")!;
    const candidates: { x: number; y: number; distance: number }[] = [];
    for (let dy = -48; dy <= 8; dy += 2) for (let dx = -32; dx <= 32; dx += 2) {
      const screenX = projected.x + dx, screenY = projected.y + dy;
      if (screenX < 2 || screenX >= canvas.width - 2 || screenY < 2 || screenY >= canvas.height - 2) continue;
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
  await page.locator("canvas").evaluate((canvas: HTMLCanvasElement) => {
    canvas.addEventListener("pointerup", (event) => {
      const pointer = event as PointerEvent;
      const rect = canvas.getBoundingClientRect();
      const screenX = (pointer.clientX - rect.left) * canvas.width / rect.width;
      const screenY = (pointer.clientY - rect.top) * canvas.height / rect.height;
      canvas.dataset.lastTerrainPick = JSON.stringify(window.gameTest.pickSurface(screenX, screenY));
    }, { capture: true, once: true });
  });
  await clickCanvas(page, point);
  await expect(page.locator("canvas")).toHaveAttribute("data-last-terrain-pick", new RegExp(`"x":${x},"y":${y}(?:,|})`));
}

async function dragWorld(page: Page, start: { x: number; y: number; z: number }, end: { x: number; y: number; z: number }): Promise<void> {
  const points = await page.evaluate(({ start, end }) => ({ start: window.gameTest.project(start), end: window.gameTest.project(end) }), { start, end });
  const from = await canvasToPage(page, points.start), to = await canvasToPage(page, points.end);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
}
