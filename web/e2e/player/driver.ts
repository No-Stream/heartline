import { expect, type Locator, type Page } from "@playwright/test";

export type ToolName = "Watch" | "Coaster" | "Paths" | "Landscape" | "Facilities" | "Scenery";

export async function selectUnits(page: Page, units: "imperial" | "metric"): Promise<void> {
  const toggle = page.getByRole("button", { name: "Units", exact: true });
  if ((await toggle.getAttribute("data-units")) !== units) await toggle.click();
  await expect(toggle).toHaveAttribute("data-units", units);
}

/** Open a tool from the toolbar, which shows its panel and switches the input mode. */
export async function openTool(page: Page, tool: ToolName): Promise<void> {
  const button = page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: tool, exact: true });
  if (await button.getAttribute("aria-pressed") !== "true") await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
}

/** Convert a point in the world canvas's backing-store pixels, as `gameTest.project` returns it, to page coordinates. */
export async function canvasToPage(page: Page, point: { x: number; y: number }): Promise<{ x: number; y: number }> {
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  const size = await canvas.evaluate((element: HTMLCanvasElement) => ({ width: element.width, height: element.height }));
  return { x: bounds.x + point.x * bounds.width / size.width, y: bounds.y + point.y * bounds.height / size.height };
}

export async function clickCanvas(page: Page, point: { x: number; y: number }): Promise<void> {
  const target = await canvasToPage(page, point);
  await page.mouse.click(target.x, target.y);
}

export async function moveOverCanvas(page: Page, point: { x: number; y: number }, options?: { steps?: number }): Promise<void> {
  const target = await canvasToPage(page, point);
  await page.mouse.move(target.x, target.y, options);
}

/** The world canvas's backing-store size, which follows the window and the open tool. */
export async function canvasSize(page: Page): Promise<{ width: number; height: number }> {
  return page.locator("canvas").evaluate((element: HTMLCanvasElement) => ({ width: element.width, height: element.height }));
}

type Tile = { x: number; y: number; z: number };
type GameSpeed = 1 | 2 | 4 | 8 | 16;

export async function startPark(page: Page, options: { park: "new" | "demo"; seed: number; speed: GameSpeed }): Promise<void> {
  await page.goto(`/?park=${options.park}&seed=${options.seed}`);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Watch");
  await setGameSpeed(page, options.speed);
}

export async function setGameSpeed(page: Page, targetSpeed: GameSpeed): Promise<void> {
  await openTool(page, "Watch");
  const speed = page.getByRole("button", { name: "Game speed" });
  for (let attempt = 0; attempt < 5; attempt++) {
    if ((await speed.textContent())?.trim() === `×${targetSpeed}`) return;
    await speed.click();
  }
  throw new Error(`Game speed ${targetSpeed} is unavailable`);
}

async function tilePoint(page: Page, tile: Tile, surfaceKind?: "terrain"): Promise<{ x: number; y: number } | null> {
  return page.evaluate(({ x, y, z, surfaceKind }) => {
    const projected = window.gameTest.project({ x: x + 0.5, y: y + 0.5, z });
    const canvas = document.querySelector("canvas")!;
    const offsets: { dx: number; dy: number }[] = [];
    for (let dy = -48; dy <= 8; dy += 2) for (let dx = -32; dx <= 32; dx += 2) offsets.push({ dx, dy });
    offsets.sort((first, second) => first.dx ** 2 + first.dy ** 2 - second.dx ** 2 - second.dy ** 2);
    for (const { dx, dy } of offsets) {
      const screenX = projected.x + dx, screenY = projected.y + dy;
      if (screenX < 2 || screenX >= canvas.width - 2 || screenY < 2 || screenY >= canvas.height - 2) continue;
      const matches = [[0, 0], [-2, 0], [2, 0], [0, -2], [0, 2]].every(([ox, oy]) => {
        const pick = window.gameTest.pickSurface(screenX + ox, screenY + oy);
        return pick && (surfaceKind ? pick.kind === surfaceKind : pick.kind === "terrain" || pick.kind === "path") && pick.x === x && pick.y === y;
      });
      if (matches) return { x: screenX, y: screenY };
    }
    return null;
  }, { ...tile, surfaceKind });
}

async function targetTiles(page: Page, tiles: readonly Tile[], surfaceKind?: "terrain"): Promise<{ x: number; y: number }[]> {
  const centerX = tiles.reduce((sum, tile) => sum + tile.x + 0.5, 0) / tiles.length;
  const centerY = tiles.reduce((sum, tile) => sum + tile.y + 0.5, 0) / tiles.length;
  const span = Math.max(...tiles.map((tile) => tile.x)) - Math.min(...tiles.map((tile) => tile.x))
    + Math.max(...tiles.map((tile) => tile.y)) - Math.min(...tiles.map((tile) => tile.y));
  const zoom = span > 16 ? 1 : 2;
  for (const rotation of [0, 1, 2, 3] as const) {
    await page.evaluate((camera) => window.gameTest.setCamera(camera), { centerX, centerY, zoom, rotation });
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    const points = await Promise.all(tiles.map((tile) => tilePoint(page, tile, surfaceKind)));
    if (points.every((point) => point !== null)) return points;
  }
  throw new Error(`Could not pick tiles ${JSON.stringify(tiles)} in any rotation`);
}

export async function clickTile(page: Page, tile: Tile): Promise<void> {
  const [point] = await targetTiles(page, [tile]);
  await clickCanvas(page, point);
}

/** Flatten each listed tile through the Landscape controls, as a player would shape station access. */
export async function flattenTerrainTiles(page: Page, tiles: readonly { x: number; y: number; height: number }[]): Promise<void> {
  await selectUnits(page, "metric");
  await openTool(page, "Landscape");
  await page.getByRole("button", { name: "Flatten", exact: true }).click();
  await page.getByLabel("Brush size").selectOption("1");
  const heightInput = page.getByLabel("Flatten height");
  const status = page.locator(".landscaping__status");
  await status.evaluate((element) => {
    element.setAttribute("data-test-updates", "0");
    const observer = new MutationObserver(() => {
      element.setAttribute("data-test-updates", String(Number(element.getAttribute("data-test-updates")) + 1));
    });
    observer.observe(element, { childList: true, characterData: true, subtree: true });
    (element as HTMLElement & { testStatusObserver: MutationObserver }).testStatusObserver = observer;
  });
  try {
    let activeHeight: number | undefined;
    for (const tile of tiles) {
      if (tile.height !== activeHeight) {
        await heightInput.fill(String(tile.height));
        await page.getByRole("button", { name: "Apply tool" }).click();
        activeHeight = tile.height;
      }
      const [point] = await targetTiles(page, [{ ...tile, z: (tile.height - 1) / 4 }], "terrain");
      const previousUpdates = Number(await status.getAttribute("data-test-updates"));
      await clickCanvas(page, point);
      await expect.poll(async () => Number(await status.getAttribute("data-test-updates")), {
        message: `flatten terrain tile (${tile.x}, ${tile.y}) to height ${tile.height}`,
      }).toBeGreaterThan(previousUpdates);
      await expect(status).toHaveText("Terrain updated");
    }
  } finally {
    await status.evaluate((element) => {
      (element as HTMLElement & { testStatusObserver: MutationObserver }).testStatusObserver.disconnect();
      element.removeAttribute("data-test-updates");
    });
  }
  await page.getByRole("button", { name: "Cancel tool" }).click();
}

type FacilityPriceName = "entrance" | "food stall" | "drink stall" | "restroom" | "carousel" | "ferris wheel" | "souvenir shop";

async function setPrice(options: {
  input: Locator;
  price: number;
}): Promise<void> {
  if (!Number.isInteger(options.price)) throw new RangeError("Price must be a whole number");
  const minimum = Number(await options.input.getAttribute("min"));
  const maximum = Number(await options.input.getAttribute("max"));
  if (options.price < minimum || options.price > maximum) throw new RangeError(`Price must be between ${minimum} and ${maximum}`);
  await options.input.fill(String(options.price));
  await options.input.press("Tab");
  await expect(options.input).toHaveValue(String(options.price));
}

/** Show one of the coaster builder's tabs: Build for track, Colours for paint, Operate for price, trains and blocks. */
export async function showBuilderTab(page: Page, tab: "Build" | "Colours" | "Operate"): Promise<void> {
  const button = page.getByRole("tablist", { name: "Coaster builder sections" }).getByRole("tab", { name: tab, exact: true });
  if (await button.getAttribute("aria-selected") !== "true") await button.click();
  await expect(button).toHaveAttribute("aria-selected", "true");
}

/** Enter the coaster ticket price in its visible control. */
export async function setCoasterTicketPrice(page: Page, price: number): Promise<void> {
  await openTool(page, "Coaster");
  await showBuilderTab(page, "Operate");
  await setPrice({
    input: page.getByRole("spinbutton", { name: "Coaster ticket price" }),
    price,
  });
}

/** Enter a facility's price in its visible control. */
export async function setFacilityPrice(page: Page, facility: { id: number; name: FacilityPriceName }, price: number): Promise<void> {
  await openTool(page, "Facilities");
  const label = `Price of ${facility.name} ${facility.id}`;
  await setPrice({
    input: page.getByRole("spinbutton", { name: label }),
    price,
  });
}

/** Frame a current coaster rider, find its rendered pick target, and select it with a real canvas click. */
export async function selectLiveRider(page: Page, riderId: string): Promise<void> {
  for (const rotation of [0, 1, 2, 3] as const) {
    await page.evaluate(({ id, rotation }) => {
      const rider = window.gameTest.riders().find((entry) => entry.id === id);
      if (!rider) throw new Error(`Rider ${id} is no longer on the coaster`);
      window.gameTest.setCamera({ centerX: rider.x - rider.z, centerY: rider.y - rider.z, zoom: 2, rotation });
    }, { id: riderId, rotation });
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    const point = await page.evaluate((id) => {
      const bounds = window.gameTest.renderedRiders().find((rider) => rider.id === id);
      if (!bounds) return null;
      for (let y = Math.ceil(bounds.top); y < bounds.bottom; y += 2) {
        for (let x = Math.ceil(bounds.left); x < bounds.right; x += 2) {
          const pick = window.gameTest.pick(x, y);
          if (pick && "id" in pick && pick.id === id) return { x, y };
        }
      }
      return null;
    }, riderId);
    if (!point) continue;
    await clickCanvas(page, point);
    if (await page.getByTestId("guest-panel").getAttribute("data-guest-id") === riderId) {
      await expect(page.locator("canvas")).toHaveAttribute("data-selection", new RegExp(`"id":"${riderId}"`));
      return;
    }
  }
  throw new Error(`Rider ${riderId} is not visible and pickable from any camera rotation`);
}

export async function dragTiles(page: Page, start: Tile, end: Tile): Promise<void> {
  const [startPoint, endPoint] = await targetTiles(page, [start, end]);
  const from = await canvasToPage(page, startPoint), to = await canvasToPage(page, endPoint);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y);
  await page.mouse.up();
}

export async function layPath(page: Page, start: Tile, end: Tile, kind: "normal" | "queue" = "normal"): Promise<void> {
  await openTool(page, "Paths");
  await page.getByLabel("Path type").selectOption(kind);
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragTiles(page, start, end);
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  await page.getByRole("region", { name: "Paths" }).getByRole("button", { name: "Cancel" }).click();
}

export async function placeFacility(page: Page, label: "Food stall" | "Drink stall" | "Restroom" | "Souvenir shop", tile: Tile): Promise<void> {
  await openTool(page, "Facilities");
  const panel = page.getByRole("region", { name: "Park facilities" });
  await panel.getByRole("radio", { name: label }).click();
  await page.getByRole("button", { name: "Place facility" }).click();
  await clickTile(page, tile);
  await expect(panel.getByRole("status")).toHaveText("Facility placed.");
}

export async function waitForState(page: Page, predicate: () => Promise<boolean>, options: { ticks: number }): Promise<void> {
  const initialTick = await page.evaluate(() => window.gameTest.tickCount());
  for (;;) {
    if (await predicate()) return;
    const currentTick = await page.evaluate(() => window.gameTest.tickCount());
    if (currentTick - initialTick >= options.ticks) throw new Error(`Condition not met within ${options.ticks} simulation ticks`);
    await page.waitForTimeout(100);
  }
}

export async function layQueue(page: Page, start: Tile, end: Tile): Promise<void> {
  await layPath(page, start, end, "queue");
}

export async function placeStation(page: Page, options: { tile: Tile; direction: Tile; height: number; model: "hybrid" | "velocity_viper" | "wooden_twister" }): Promise<void> {
  await selectUnits(page, "metric");
  await openTool(page, "Coaster");
  await page.getByRole("combobox", { name: "Coaster model" }).selectOption(`model:${options.model}`);
  const height = page.getByLabel("Station height above ground (metres)");
  await height.fill(String(options.height));
  await height.press("Tab");
  await expect(height).toHaveValue(String(options.height));
  await page.getByRole("button", { name: "Place station" }).click();
  const [station, direction] = await targetTiles(page, [options.tile, options.direction]);
  await clickCanvas(page, station);
  await clickCanvas(page, direction);
  await expect(page.locator("[data-track] li")).toHaveCount(1);
}

type PieceStep =
  | { attribute: "turn"; direction: "left" | "right" }
  | { attribute: "slope"; direction: "down" | "up" }
  | { attribute: "bank"; direction: "left" | "right" };

export async function choosePiece(page: Page, steps: readonly PieceStep[], expectedName: string): Promise<void> {
  const labels = { turn: "Turn", slope: "Slope", bank: "Bank" } as const;
  for (const step of steps) {
    await page.getByRole("button", { name: `${labels[step.attribute]} ${step.direction}`, exact: true }).click();
  }
  await expect(page.locator("[data-preview-name]")).toHaveText(expectedName);
}

export async function placePiece(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Place piece" }).click();
}

export async function closeCircuit(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Close circuit" }).click();
}

export async function testRide(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Test coaster" }).click();
}

export async function openRide(page: Page): Promise<void> {
  await showBuilderTab(page, "Build");
  await page.getByRole("button", { name: "Open coaster" }).click();
}
