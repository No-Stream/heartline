import { expect, test, type Page } from "@playwright/test";
import { canvasToPage, clickCanvas, clickTile, moveOverCanvas, openTool, selectUnits } from "./game-page";

const blockCircuitPieces = [
  "Straight", "Straight", "Straight", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit",
  "Straight", "Straight", "Straight", "Straight", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit",
];

const boardingCircuitPieces = [
  "LevelToGentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUpToLevel", "Straight",
  "Straight", "Straight", "OffsetLeft", "TurnLargeEntry", "TurnLargeExit", "LevelToGentleDown", "GentleDown", "GentleDown",
  "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel", "Loop", "OffsetRight", "TurnLargeEntry", "TurnLargeExit",
];

test("automatic block sections can be inspected, moved, refused, and rendered @slow", async ({ page }) => {
  test.setTimeout(300_000);
  await buildBlockCircuit(page, 50, 50, 20);

  const blockPanel = page.getByRole("region", { name: "Block sections" });
  await expect(blockPanel).toBeVisible();
  await page.getByRole("button", { name: "Add block sections" }).click();
  await installVerifiedPlacement(page);

  const report = blockPanel.locator("[data-block-report]");
  const capacity = blockPanel.locator("[data-block-capacity]");
  await expect(capacity).toHaveText(/[1-9]/);
  const editableRows = page.locator("[data-block-section]:has([data-block-boundary])");
  await expect(editableRows).not.toHaveCount(0);
  await expect(blockPanel.locator('[data-train-count]')).toBeVisible();

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 58, centerY: 54, zoom: 0.45, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const canvas = page.locator("canvas");
  await page.mouse.move(0, 0);
  await expect(canvas).toHaveScreenshot("automatic-block-markers.png", { animations: "disabled" });

  const editableRow = editableRows.first();
  const editableIndex = await editableRow.getAttribute("data-block-index");
  if (editableIndex === null) throw new Error("Editable block section has no index");
  const boundary = editableRow.locator("[data-block-boundary]");
  const installedBoundary = await boundary.inputValue();
  const marker = await findBlockMarker(page, Number(editableIndex));
  await clickCanvas(page, marker);
  await boundary.fill("0.123456");
  await editableRow.locator(`[data-block-move="${editableIndex}"]`).click();
  await expect(blockPanel.locator("[data-block-message]")).toContainText(/Choose a listed level brake run/);
  await expect(boundary).toHaveValue(installedBoundary);
  const rows = page.locator("[data-block-section]:has([data-block-boundary])");
  const count = await rows.count();
  expect(count, "automatic placement must expose more than one editable brake run").toBeGreaterThan(1);
  const removedRow = rows.nth(count - 1);
  const removedIndex = await removedRow.getAttribute("data-block-index");
  if (removedIndex === null || removedIndex === "") throw new Error("Editable block section has no index");
  const destination = await removedRow.locator("[data-block-boundary]").inputValue();
  await removedRow.locator(`[data-block-remove="${removedIndex}"]`).click();
  await finishManualValidation(page);
  await expect(rows).toHaveCount(count - 1);

  const remainingRow = rows.first();
  const remainingIndex = await remainingRow.getAttribute("data-block-index");
  if (remainingIndex === null || remainingIndex === "") throw new Error("Remaining block section has no index");
  await remainingRow.locator("[data-block-boundary]").fill(destination);
  await remainingRow.locator(`[data-block-move="${remainingIndex}"]`).click();
  await finishManualValidation(page);
  const installedBoundaries = await page.locator("[data-block-boundary]").evaluateAll((elements) =>
    elements.map((element) => (element as HTMLInputElement).value));
  expect(installedBoundaries).toContain(destination);
  await expect(report).toHaveAttribute("data-placement-status", "manual");
});

test("two requested block trains move visibly without overlapping @slow", async ({ page }) => {
  test.setTimeout(300_000);
  await buildBlockCircuit(page, 241, 249, 4, "north", boardingCircuitPieces, 48);

  const blockPanel = page.getByRole("region", { name: "Block sections" });
  await page.getByRole("button", { name: "Add block sections" }).click();
  await installVerifiedPlacement(page);
  const capacity = await blockPanel.locator("[data-block-capacity]").textContent();
  const availableTrains = Number(/up to (\d+) trains?/.exec(capacity ?? "")?.[1] ?? 0);
  expect(availableTrains, "the automatic layout should support at least two trains").toBeGreaterThanOrEqual(2);

  const trainCount = blockPanel.locator("[data-train-count]");
  while (Number(await trainCount.textContent()) > 1) {
    await page.getByRole("button", { name: "Run one fewer train" }).click();
  }
  await expect(page.getByRole("button", { name: "Run one fewer train" })).toBeDisabled();
  await page.getByRole("button", { name: "Run one more train" }).click();
  await expect(trainCount).toHaveText("2");

  await setStationAccessSurfaces(page);
  await openTool(page, "Paths");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 241, centerY: 253, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await buildAccessPaths(page);
  await openTool(page, "Coaster");
  await page.getByRole("button", { name: "Open coaster" }).click();
  await expect(page.getByRole("button", { name: "Close coaster" })).toBeVisible();

  let movingTrains: TrainGroup[] | undefined;
  for (let attempt = 0; attempt < 300 && !movingTrains; attempt += 1) {
    await advanceTicks(page, 100);
    const before = await readTrainGroups(page);
    if (before.length < 2) continue;
    await advanceTicks(page, 100);
    const after = await readTrainGroups(page);
    const moving = before.filter((train) => {
      const next = after.find((candidate) => candidate.id === train.id);
      return next !== undefined && trainMoved(train, next);
    });
    if (moving.length >= 2) movingTrains = after.filter((train) => moving.some((entry) => entry.id === train.id));
  }

  if (!movingTrains) throw new Error("Two active trains did not move during the same interval");
  expect(movingTrains).toHaveLength(2);
  const carPositions = movingTrains.flatMap(({ cars }) => cars);
  for (let first = 0; first < movingTrains[0].cars.length; first += 1) {
    for (let second = 0; second < movingTrains[1].cars.length; second += 1) {
      const firstCar = movingTrains[0].cars[first];
      const secondCar = movingTrains[1].cars[second];
      expect(distanceBetween(firstCar, secondCar), "cars from separate trains must keep a visible gap").toBeGreaterThan(0.5);
    }
  }
  const cameraCenter = carPositions.reduce((center, car) => ({ x: center.x + car.x, y: center.y + car.y }), { x: 0, y: 0 });
  cameraCenter.x /= carPositions.length;
  cameraCenter.y /= carPositions.length;
  await page.evaluate((center) => window.gameTest.setCamera({ centerX: center.x, centerY: center.y, zoom: 0.45, rotation: 0 }), cameraCenter);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const visibleTrainIds = await findVisibleTrainIds(page, carPositions);
  expect(visibleTrainIds).toEqual(expect.arrayContaining(movingTrains.map(({ id }) => id)));
});

interface TrainCar {
  readonly carId: number;
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

interface TrainGroup {
  readonly id: number;
  readonly cars: readonly TrainCar[];
}

async function buildBlockCircuit(
  page: Page,
  x: number,
  y: number,
  height: number,
  orientation: "east" | "north" = "east",
  pieces: readonly string[] = blockCircuitPieces,
  agentCount = 12,
): Promise<void> {
  const pageErrors: string[] = [];
  const failedResources: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("response", (response) => { if (response.status() >= 400) failedResources.push(`${response.status()} ${response.url()}`); });
  page.on("requestfailed", (request) => failedResources.push(`${request.failure()?.errorText ?? "request failed"} ${request.url()}`));
  await page.goto(agentCount === 12 ? "/?test=1&manualBlockAnalysis=1" : `/?test=1&manualBlockAnalysis=1&agents=${agentCount}`);
  try {
    await page.waitForFunction(() => window.gameTest?.ready ?? false, null, { timeout: 30_000 });
  } catch (error) {
    const startupState = await page.evaluate(() => ({
      href: location.href,
      readyState: document.readyState,
      gameTestType: typeof window.gameTest,
      scripts: Array.from(document.scripts, (script) => script.src),
      resources: performance.getEntriesByType("resource").map(({ name }) => name),
    }));
    throw new Error(`Browser did not become ready: ${JSON.stringify({ startupState, pageErrors, failedResources })}`, { cause: error });
  }
  if (pageErrors.length > 0) throw new Error(`Browser startup errors: ${pageErrors.join(" | ")}`);
  if (failedResources.length > 0) throw new Error(`Game assets failed to load: ${failedResources.join(" | ")}`);
  await selectUnits(page, "metric");
  await openTool(page, "Coaster");
  await page.evaluate(({ x, y }) => window.gameTest.setCamera({ centerX: x, centerY: y, zoom: 1, rotation: 0 }), { x, y });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const stationHeight = page.getByLabel("Station height above ground (metres)");
  await stationHeight.fill(String(height));
  await stationHeight.press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();

  await clickTile(page, { x, y, z: 2 });
  const heading = orientation === "east"
    ? { x: x + 2.5, y: y + 0.5, z: 2 }
    : { x: x + 0.5, y: y + 2.5, z: 2 };
  const direction = await page.evaluate((position) => window.gameTest.project(position), heading);
  await moveOverCanvas(page, direction);
  await clickCanvas(page, direction);

  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  let chainEnabled = false;
  for (const [index, piece] of pieces.entries()) {
    const needsChain = pieces === blockCircuitPieces ? index === 2 || index === 9 : index < 8;
    if (needsChain !== chainEnabled) {
      await page.keyboard.press("h");
      chainEnabled = needsChain;
    }
    const choice = choices.locator(`[data-piece="${piece}"]`);
    await choice.click();
    await expect(choice).toHaveAttribute("aria-checked", "true");
    const placePiece = page.getByRole("button", { name: "Place piece" });
    if (!(await placePiece.isEnabled())) {
      const refusal = await choice.locator('[role="tooltip"]').textContent();
      throw new Error(`Cannot place ${piece} at circuit index ${index + 1}: ${refusal ?? "no refusal reason was reported"}`);
    }
    await placePiece.click();
    await expect(track.locator("li")).toHaveCount(index + 2);
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
}

async function installVerifiedPlacement(page: Page): Promise<void> {
  const report = page.locator("[data-block-report]");
  const capacity = page.locator("[data-block-capacity]");
  await expect(report).toHaveAttribute("data-placement-status", /^(searching|incomplete|complete)$/);
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const placementStatus = await report.getAttribute("data-placement-status");
    const installedCapacity = Number(/supports up to (\d+) trains?/.exec(await capacity.textContent() ?? "")?.[1] ?? 0);
    if (placementStatus !== "searching") {
      expect(installedCapacity, "automatic placement must verify at least two trains").toBeGreaterThanOrEqual(2);
      return;
    }
    if (installedCapacity >= 2) {
      await page.getByRole("button", { name: "Use current layout" }).click();
      await expect(report).toHaveAttribute("data-placement-status", /^(incomplete|complete)$/);
      await expect(page.getByRole("button", { name: "Use current layout" })).toBeHidden();
      const selectedCapacity = Number(/supports up to (\d+) trains?/.exec(await capacity.textContent() ?? "")?.[1] ?? 0);
      expect(selectedCapacity, "using the current layout must retain its verified capacity").toBeGreaterThanOrEqual(installedCapacity);
      return;
    }
    await advanceBlockAnalysis(page);
  }
  throw new Error("Automatic placement did not verify two trains within 4,000,000 analysis steps");
}

async function findBlockMarker(page: Page, index: number): Promise<{ x: number; y: number }> {
  const marker = await page.evaluate((boundaryIndex) => {
    const canvas = document.querySelector("canvas");
    if (!canvas) throw new Error("World canvas is missing");
    for (let y = 2; y < canvas.height; y += 2) {
      for (let x = 2; x < canvas.width; x += 2) {
        const pick = window.gameTest.pick(x, y);
        if (pick?.kind === "block-boundary" && pick.id === `block-boundary-${boundaryIndex}`) return { x, y };
      }
    }
    return null;
  }, index);
  if (!marker) throw new Error(`Block boundary ${index} is not visible and pickable`);
  return marker;
}

async function finishManualValidation(page: Page): Promise<void> {
  const report = page.locator("[data-block-report]");
  await expect(report).toHaveAttribute("data-placement-status", "validating");
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (await report.getAttribute("data-placement-status") === "manual") return;
    await advanceBlockAnalysis(page);
  }
  await expect(report).toHaveAttribute("data-placement-status", "manual");
}

async function advanceBlockAnalysis(page: Page): Promise<void> {
  await page.evaluate(async () => {
    for (let slice = 0; slice < 20; slice += 1) await window.gameTest.advanceBlockAnalysis(1000);
  });
}

async function setStationAccessSurfaces(page: Page): Promise<void> {
  await openTool(page, "Landscape");
  await page.getByRole("button", { name: "Flatten", exact: true }).click();
  await page.getByLabel("Brush size").selectOption("1");
  for (const [y, height] of [[255, 8], [254, 9], [253, 10], [252, 11], [251, 12], [250, 12]] as const) {
    await page.getByLabel("Flatten height").fill(String(height));
    for (const x of [238, 244]) {
      await page.getByRole("button", { name: "Apply tool" }).click();
      await clickTerrain(page, x, y);
      await expect(page.locator(".landscaping__status")).toHaveText("Terrain updated");
    }
  }
  await page.getByLabel("Flatten height").fill("12");
  for (const x of [239, 243]) {
    await page.getByRole("button", { name: "Apply tool" }).click();
    await clickTerrain(page, x, 250);
    await expect(page.locator(".landscaping__status")).toHaveText("Terrain updated");
  }
  await page.getByRole("button", { name: "Cancel tool" }).click();
}

async function buildAccessPaths(page: Page): Promise<void> {
  const pathsPanel = page.getByRole("region", { name: "Paths" });
  await page.getByLabel("Path type").selectOption("queue");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragWorld(page, { x: 243.5, y: 250.5, z: 3 }, { x: 244.5, y: 256.5, z: 2 });
  await expect(pathsPanel.getByRole("status")).toHaveText("Path built.");
  await pathsPanel.getByRole("button", { name: "Cancel" }).click();

  await page.getByLabel("Path type").selectOption("normal");
  await page.getByRole("button", { name: "Build paths" }).click();
  await dragWorld(page, { x: 239.5, y: 250.5, z: 3 }, { x: 238.5, y: 256.5, z: 2 });
  await expect(pathsPanel.getByRole("status")).toHaveText("Path built.");
  await pathsPanel.getByRole("button", { name: "Cancel" }).click();
}

async function dragWorld(
  page: Page,
  start: { x: number; y: number; z: number },
  end: { x: number; y: number; z: number },
): Promise<void> {
  const points = await page.evaluate(({ start, end }) => ({ start: window.gameTest.project(start), end: window.gameTest.project(end) }), { start, end });
  const from = await canvasToPage(page, points.start);
  const to = await canvasToPage(page, points.end);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
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
        const screenX = projected.x + dx;
        const screenY = projected.y + dy;
        if (screenX < 2 || screenX >= canvas.width - 2 || screenY < 2 || screenY >= canvas.height - 2) continue;
        const matches = [[0, 0], [-2, 0], [2, 0], [0, -2], [0, 2]].every(([offsetX, offsetY]) => {
          const pick = window.gameTest.pickSurface(screenX + offsetX, screenY + offsetY);
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

async function advanceTicks(page: Page, count: number): Promise<void> {
  const tickCount = page.getByTestId("tick-count");
  const previousTickCount = await tickCount.textContent();
  if (previousTickCount === null) throw new Error("Simulation tick counter is missing");
  await page.evaluate((ticks) => window.gameTest.advanceTicks(ticks), count);
  await expect(tickCount).not.toHaveText(previousTickCount);
}

async function readTrainGroups(page: Page): Promise<TrainGroup[]> {
  return page.evaluate(() => {
    const poses = window.gameTest.constructionTrainPoses();
    if (poses.length % 11 !== 0) throw new Error("Invalid construction train pose stride");
    const groups = new Map<number, TrainCar[]>();
    for (let offset = 0; offset < poses.length; offset += 11) {
      const carId = poses[offset];
      const trainId = Math.floor(carId / 6);
      const cars = groups.get(trainId) ?? [];
      cars.push({ carId, x: poses[offset + 1], y: poses[offset + 2], z: poses[offset + 3] });
      groups.set(trainId, cars);
    }
    return Array.from(groups, ([id, cars]) => ({ id, cars }));
  });
}

async function findVisibleTrainIds(page: Page, cars: readonly TrainCar[]): Promise<number[]> {
  return page.evaluate((trainCars) => {
    const visible = new Set<number>();
    for (const car of trainCars) {
      const center = window.gameTest.project({ x: car.x, y: car.y, z: car.z });
      for (let dy = -8; dy <= 8; dy += 2) for (let dx = -8; dx <= 8; dx += 2) {
        const pick = window.gameTest.pick(center.x + dx, center.y + dy);
        if (pick?.kind === "train" && pick.id === `train-${car.carId}`) visible.add(Math.floor(car.carId / 6));
      }
    }
    return Array.from(visible);
  }, cars);
}

function trainMoved(before: TrainGroup, after: TrainGroup): boolean {
  return before.cars.some((car, index) => distanceBetween(car, after.cars[index]) > 0.02);
}

function distanceBetween(first: TrainCar, second: TrainCar): number {
  return Math.hypot(first.x - second.x, first.y - second.y, first.z - second.z);
}
