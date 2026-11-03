import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, expect } from "@playwright/test";

const baseUrl = process.env.BASE_URL ?? "http://127.0.0.1:4740";
const sessionDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "sessions");
const browser = await chromium.launch({ channel: "chrome" });
const context = await browser.newContext({ acceptDownloads: true, viewport: { width: 1280, height: 720 } });
const page = await context.newPage();
const browserErrors = [];
page.on("pageerror", (error) => browserErrors.push(error.message));
page.on("console", (message) => {
  if (message.type() === "error") browserErrors.push(message.text());
});
page.on("response", (response) => {
  if (response.status() >= 400) console.log(`HTTP ${response.status()} ${response.url()}`);
});

try {
  await mkdir(sessionDirectory, { recursive: true });
  await loadNewPark();
  await recordTimberTumbler();
  await loadNewPark();
  await recordPathsAndFoodStall();
  console.log(`Browser console errors: ${JSON.stringify(browserErrors)}`);
  expect(browserErrors).toEqual([]);
} finally {
  await context.close();
  await browser.close();
}

async function loadNewPark() {
  await page.goto(`${baseUrl}/?test=1&park=new&seed=2026`);
  await page.waitForFunction(() => window.gameTest?.ready === true, undefined, { timeout: 30_000 });
}

async function centerCamera(centerX, centerY) {
  await page.evaluate(({ x, y }) => {
    window.gameTest.setCamera({ centerX: x, centerY: y, zoom: 2, rotation: 0 });
  }, { x: centerX, y: centerY });
  await page.waitForFunction(() => window.gameTest.ready === true, undefined, { timeout: 30_000 });
}

async function pointForTile(tile) {
  const point = await page.evaluate(({ x, y, z }) => {
    const projected = window.gameTest.project({ x: x + 0.5, y: y + 0.5, z });
    const canvas = document.querySelector("canvas");
    if (!canvas) throw new Error("Game canvas is missing");
    const candidates = [];
    for (let dy = -48; dy <= 8; dy += 2) {
      for (let dx = -32; dx <= 32; dx += 2) {
        const screenX = projected.x + dx;
        const screenY = projected.y + dy;
        if (screenX < 2 || screenX >= canvas.width - 2 || screenY < 2 || screenY >= canvas.height - 2) continue;
        const matches = [[0, 0], [-2, 0], [2, 0], [0, -2], [0, 2]].every(([offsetX, offsetY]) => {
          const picked = window.gameTest.pickSurface(screenX + offsetX, screenY + offsetY);
          return picked && (picked.kind === "terrain" || picked.kind === "path") && picked.x === x && picked.y === y;
        });
        if (matches) candidates.push({ x: screenX, y: screenY, distance: dx * dx + dy * dy });
      }
    }
    candidates.sort((first, second) => first.distance - second.distance);
    return candidates[0] ?? null;
  }, tile);
  if (!point) throw new Error(`Could not pick tile ${tile.x},${tile.y}`);
  return point;
}

async function canvasPointToPage(point) {
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Game canvas is not visible");
  const size = await canvas.evaluate((element) => ({ width: element.width, height: element.height }));
  return {
    x: bounds.x + point.x * bounds.width / size.width,
    y: bounds.y + point.y * bounds.height / size.height,
  };
}

async function clickCanvasPoint(point) {
  const target = await canvasPointToPage(point);
  await page.mouse.move(target.x, target.y);
  await page.mouse.click(target.x, target.y);
}

async function dragCanvasPoints(start, end) {
  const from = await canvasPointToPage(start);
  const to = await canvasPointToPage(end);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
}

async function exportSession(fileName, expectedState) {
  await page.evaluate(() => window.gameTest.advanceTicks(300));
  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Watch", exact: true }).click();
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export session", exact: true }).click();
  const download = await downloadPromise;
  const exportedPath = await download.path();
  if (!exportedPath) throw new Error("Session export has no downloaded file");

  const session = JSON.parse(await readFile(exportedPath, "utf8"));
  expect(session.format).toBe("theme-park-session");
  expect(session.version).toBe(1);
  expect(session.finalTick).toBe(300);
  expect(session.checkpoints).toContainEqual({ tick: 300, hash: expect.any(String) });
  session.expect = expectedState;
  const outputPath = resolve(sessionDirectory, fileName);
  await writeFile(outputPath, `${JSON.stringify(session, null, 2)}\n`);
  console.log(`${outputPath}: ${session.commands.length} commands, ${session.inputs.length} input actions, tick ${session.finalTick}`);
}

async function recordTimberTumbler() {
  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Coaster", exact: true }).click();
  await centerCamera(241, 250);
  await page.getByRole("button", { name: "Place station" }).click();
  const station = await pointForTile({ x: 241, y: 249, z: 2 });
  const direction = await pointForTile({ x: 241, y: 251, z: 2 });
  await page.locator("canvas").hover({ position: { x: station.x, y: station.y } });
  await clickCanvasPoint(station);
  await page.locator("canvas").hover({ position: { x: direction.x, y: direction.y } });
  await clickCanvasPoint(direction);
  await expect(page.locator("[data-track] li")).toHaveCount(1);

  await page.keyboard.press("h");
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "true");
  await page.getByRole("radio", { name: "Begin gentle climb", exact: true }).click();
  for (const count of [2, 3, 4]) {
    if (count > 2) await page.getByRole("radio", { name: "Gentle climb", exact: true }).click();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(page.locator("[data-track] li")).toHaveCount(count);
  }
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "true");
  await exportSession("timber-tumbler-chain-lift.json", {
    constructionPiecesAtLeast: 4,
    chainEnabled: true,
  });
}

async function recordPathsAndFoodStall() {
  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Paths", exact: true }).click();
  await centerCamera(218, 253);
  await page.getByRole("button", { name: "Build paths" }).click();
  const pathStart = await pointForTile({ x: 218, y: 251, z: 2 });
  const pathEnd = await pointForTile({ x: 218, y: 256, z: 2 });
  await dragCanvasPoints(pathStart, pathEnd);
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");

  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Facilities", exact: true }).click();
  await centerCamera(217, 253);
  const facilityPanel = page.getByRole("region", { name: "Park facilities" });
  await facilityPanel.getByRole("radio", { name: "Food stall" }).click();
  await facilityPanel.getByRole("button", { name: "Place facility" }).click();
  await clickCanvasPoint(await pointForTile({ x: 216, y: 252, z: 2 }));
  await expect(facilityPanel.getByRole("status")).toHaveText("Facility placed.");
  await exportSession("paths-food-stall.json", {
    pathTilesAtLeast: 6,
    foodStallsAtLeast: 1,
  });
}
