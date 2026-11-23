import { expect, type Locator, type Page } from "@playwright/test";
import type { CoasterModelKey } from "../../src/generated/coaster-models";

export type ToolName = "Watch" | "Coaster" | "Paths" | "Landscape" | "Facilities" | "Scenery";

export interface RideRating {
  readonly excitement: number;
  readonly intensity: number;
  readonly nausea: number;
}

/** Convert the new scores to the previous numeric scale for calibration logs. */
export function legacyRideRating(rating: RideRating): RideRating {
  return {
    excitement: rating.excitement * 10,
    intensity: rating.intensity * 10,
    nausea: rating.nausea * 10,
  };
}

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

/** Fail on the normal assertion timeout when a button is missing or unavailable. */
export async function clickVisibleButton(button: Locator): Promise<void> {
  await expect(button).toBeVisible();
  await expect(button).toBeEnabled();
  await button.click();
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

export async function startPark(page: Page, options: { park: "new" | "demo"; seed: number; speed: GameSpeed; livePauseRender?: boolean; testMode?: boolean; agents?: number }): Promise<void> {
  if (options.agents !== undefined && (!Number.isInteger(options.agents) || options.agents < 1 || options.agents > 500)) {
    throw new RangeError("Guest count must be an integer from 1 through 500");
  }
  const path = `/?park=${options.park}&seed=${options.seed}${options.livePauseRender ? "&livePauseRender=1" : ""}${options.testMode ? "&test=1" : ""}${options.agents === undefined ? "" : `&agents=${options.agents}`}`;
  await gotoGame(page, path);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Watch");
  await setGameSpeed(page, options.speed);
}

export async function gotoGame(page: Page, path: string): Promise<void> {
  const url = new URL(path, "http://localhost");
  if (process.env.THEME_PARK_LIVE_PAUSE_RENDER === "1") url.searchParams.set("livePauseRender", "1");
  await page.goto(`${url.pathname}${url.search}${url.hash}`);
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
  const zooms = span > 16 ? [1, 0.75, 0.5, 1.5, 2] as const : [2, 1.5, 1, 0.75, 0.5] as const;
  for (const rotation of [0, 1, 2, 3] as const) {
    for (const zoom of zooms) {
      await page.evaluate((camera) => window.gameTest.setCamera(camera), { centerX, centerY, zoom, rotation });
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      const points = await Promise.all(tiles.map((tile) => tilePoint(page, tile, surfaceKind)));
      if (points.every((point) => point !== null)) return points;
    }
  }
  throw new Error(`Could not pick tiles ${JSON.stringify(tiles)} in any rotation or zoom`);
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
      const [updatedPoint] = await targetTiles(page, [{ ...tile, z: (tile.height - 1) / 4 }], "terrain");
      const updatedTerrain = await page.evaluate((position) => window.gameTest.pickSurface(position.x, position.y), updatedPoint);
      expect(updatedTerrain, `terrain tile (${tile.x}, ${tile.y}) height`).toMatchObject({
        kind: "terrain",
        x: tile.x,
        y: tile.y,
        height: tile.height,
      });
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
  const editingCoasterId = await page.evaluate(() => window.gameTest.editingCoasterId());
  if (editingCoasterId === null) throw new Error("No coaster is being edited");
  await selectCoasterById(page, editingCoasterId);
  await setPrice({ input: page.getByTestId("ride-price"), price });
}

/** Leave the builder so the coaster list is available. */
export async function stopCoasterEditing(page: Page): Promise<void> {
  const button = page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Coaster", exact: true });
  if (await button.getAttribute("aria-pressed") === "true") await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "false");
}

/** Select a coaster in the park list and open its ride window. */
export async function selectCoasterByName(page: Page, name: string): Promise<void> {
  await closeRideWindow(page);
  await stopCoasterEditing(page);
  await openTool(page, "Coaster");
  const entry = page.locator(".coaster-list").getByRole("button", { name: new RegExp(`^View ${escapeRegExp(name)},`) });
  await expect(entry).toBeVisible();
  const coasterId = await entry.getAttribute("data-coaster-id");
  if (coasterId === null) throw new Error(`Coaster list entry for ${name} has no stable id`);
  await clickVisibleButton(entry);
  await expect(page.getByTestId("ride-window")).toBeVisible();
  await expect(page.getByTestId("ride-window")).toHaveAttribute("data-ride", coasterId);
}

/** Rename the coaster shown in the ride window. */
export async function renameSelectedCoaster(page: Page, name: string): Promise<void> {
  await clickVisibleButton(page.getByTestId("ride-rename-toggle"));
  const nameInput = page.getByTestId("ride-name-input");
  await expect(nameInput).toBeVisible();
  await nameInput.fill(name);
  await clickVisibleButton(page.getByTestId("ride-rename-submit"));
  await expect(page.getByTestId("ride-name")).toHaveText(name);
}

/** Select a coaster by stable id in the park list and open its ride window. */
async function selectCoasterById(page: Page, coasterId: number): Promise<void> {
  await closeRideWindow(page);
  await stopCoasterEditing(page);
  await openTool(page, "Coaster");
  const entry = page.locator(`.coaster-list [data-coaster-id="${coasterId}"]`);
  await expect(entry).toBeVisible();
  await clickVisibleButton(entry);
  await expect(page.getByTestId("ride-window")).toBeVisible();
  await expect(page.getByTestId("ride-window")).toHaveAttribute("data-ride", String(coasterId));
}

/** Select a rendered track piece from the world so the ride window follows its coaster. */
export async function selectCoasterFromWorld(page: Page, coasterId: number, camera: { centerX: number; centerY: number }): Promise<void> {
  await page.evaluate(({ centerX, centerY }) => window.gameTest.setCamera({ centerX, centerY, zoom: 1.5, rotation: 0 }), camera);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const target = await page.evaluate((id) => {
    const canvas = document.querySelector("canvas");
    if (!canvas) return null;
    for (let y = 0; y < canvas.height; y += 2) {
      for (let x = 0; x < canvas.width; x += 2) {
        const pick = window.gameTest.pick(x, y);
        if (pick?.kind === "track" && "id" in pick && pick.id.startsWith(`track-${id}-`)) return { x, y };
      }
    }
    return null;
  }, coasterId);
  if (!target) throw new Error(`Coaster ${coasterId} has no visible track at the selected camera`);
  await clickCanvas(page, target);
  await expect(page.getByTestId("ride-window")).toBeVisible();
  await expect(page.getByTestId("ride-window")).toHaveAttribute("data-ride", String(coasterId));
}

export async function closeRideWindow(page: Page): Promise<void> {
  const rideWindow = page.getByTestId("ride-window");
  if (await rideWindow.isVisible()) await clickVisibleButton(rideWindow.getByTestId("ride-window-close"));
  await expect(rideWindow).toBeHidden();
}

export async function finishCoaster(page: Page): Promise<void> {
  const finish = page.getByRole("button", { name: "Finish coaster", exact: true });
  if (await finish.isVisible() && await finish.isEnabled()) {
    await clickVisibleButton(finish);
    const confirm = page.getByRole("button", { name: "Confirm coaster finish", exact: true });
    if (await confirm.isVisible()) await clickVisibleButton(confirm);
  }
  await expect(page.getByRole("button", { name: /^Test coaster(?:\s|$)/ })).toBeEnabled();
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

export async function advanceUntilState(page: Page, predicate: () => Promise<boolean>, options: { ticks: number }): Promise<void> {
  const advance = async (count: number): Promise<void> => {
    await page.evaluate((ticks) => window.gameTest.advanceTicks(ticks), count);
    await page.evaluate(() => new Promise<void>((resolve) => {
      const waitForFrame = (): void => {
        if (window.gameTest.ready) resolve();
        else requestAnimationFrame(waitForFrame);
      };
      waitForFrame();
    }));
  };
  await advance(0);
  const initialTick = await page.evaluate(() => window.gameTest.tickCount());
  for (;;) {
    if (await predicate()) return;
    const elapsedTicks = await page.evaluate(() => window.gameTest.tickCount()) - initialTick;
    if (elapsedTicks >= options.ticks) throw new Error(`Condition not met within ${options.ticks} simulation ticks`);
    await advance(Math.min(64, options.ticks - elapsedTicks));
  }
}

export async function layQueue(page: Page, start: Tile, end: Tile): Promise<void> {
  await layPath(page, start, end, "queue");
}

export async function placeStation(page: Page, options: { tile: Tile; direction: Tile; height: number; model: CoasterModelKey }): Promise<void> {
  await selectUnits(page, "metric");
  await openTool(page, "Coaster");
  const newCoaster = page.getByRole("button", { name: "New coaster", exact: true });
  if (!(await newCoaster.isVisible())) {
    const coasterTool = page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Coaster", exact: true });
    if (await coasterTool.getAttribute("aria-pressed") === "true") await coasterTool.click();
    await openTool(page, "Coaster");
    await expect(newCoaster).toBeVisible();
  }
  await clickVisibleButton(newCoaster);
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
  const rideWindow = page.getByTestId("ride-window");
  if (!(await rideWindow.isVisible())) {
    const editingCoasterId = await page.evaluate(() => window.gameTest.editingCoasterId());
    if (editingCoasterId === null) throw new Error("No coaster is being edited or selected");
    await selectCoasterById(page, editingCoasterId);
  }
  const openToggle = page.getByTestId("ride-open-toggle");
  const openButton = page.getByRole("button", { name: "Open coaster", exact: true });
  if (await openButton.isVisible()) await openButton.click();
  await expect(openToggle).toHaveAccessibleName("Close coaster");
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
