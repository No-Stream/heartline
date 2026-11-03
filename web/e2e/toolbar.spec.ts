import { expect, test, type Page } from "@playwright/test";
import { canvasSize, canvasToPage, moveOverCanvas, openTool } from "./game-page";

const panels = {
  Coaster: "Coaster construction",
  Paths: "Paths",
  Landscape: "Landscaping",
  Facilities: "Park facilities",
} as const;
const barHeight = 44;
const dockWidth = 400;

async function startGame(page: Page): Promise<void> {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
}

async function visiblePanels(page: Page): Promise<string[]> {
  const visible: string[] = [];
  for (const name of Object.values(panels)) if (await page.getByRole("region", { name, exact: true }).isVisible()) visible.push(name);
  return visible;
}

test.describe("tool switching", () => {
  test.use({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });

  test("opens one panel at a time beside a world that fills the rest of the window", async ({ page }) => {
    await startGame(page);
    await expect(page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Watch", exact: true })).toHaveAttribute("aria-pressed", "true");
    expect(await visiblePanels(page)).toEqual([]);
    expect(await canvasSize(page)).toEqual({ width: 1280, height: 720 - barHeight });

    for (const [tool, panel] of Object.entries(panels) as [keyof typeof panels, string][]) {
      await openTool(page, tool);
      expect(await visiblePanels(page)).toEqual([panel]);
      expect(await canvasSize(page)).toEqual({ width: 1280 - dockWidth, height: 720 - barHeight });
      const panelBounds = await page.getByRole("region", { name: panel, exact: true }).boundingBox();
      expect(panelBounds?.x).toBeGreaterThanOrEqual(1280 - dockWidth);
      expect((panelBounds?.x ?? 0) + (panelBounds?.width ?? 0)).toBeLessThanOrEqual(1280);
    }

    await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Facilities", exact: true }).click();
    expect(await visiblePanels(page)).toEqual([]);
    expect(await canvasSize(page)).toEqual({ width: 1280, height: 720 - barHeight });
  });

  test("keeps the top-left of the park still when a tool opens", async ({ page }) => {
    await startGame(page);
    const before = await page.evaluate(() => window.gameTest.project({ x: 240, y: 250, z: 2 }));
    await openTool(page, "Paths");
    const after = await page.evaluate(() => window.gameTest.project({ x: 240, y: 250, z: 2 }));
    expect(after.x).toBeCloseTo(before.x, 5);
    expect(after.y).toBeCloseTo(before.y, 5);
  });

  test("switches tools from the keyboard and leaves an input mode before closing the tool", async ({ page }) => {
    await startGame(page);
    const tools = page.getByRole("toolbar", { name: "Tools" });
    await page.locator("canvas").focus();
    for (const [key, tool] of [["o", "Coaster"], ["l", "Landscape"], ["f", "Facilities"], ["p", "Paths"]] as const) {
      await page.keyboard.press(key);
      await expect(tools.getByRole("button", { name: tool, exact: true })).toHaveAttribute("aria-pressed", "true");
    }
    const pathStatus = page.getByRole("region", { name: "Paths" }).getByRole("status");
    await expect(pathStatus).toHaveText("Drag between tiles to build a path.");

    await page.keyboard.press("Escape");
    await expect(pathStatus).toHaveText("Path selection cancelled.");
    await expect(tools.getByRole("button", { name: "Paths", exact: true })).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press("Escape");
    await expect(tools.getByRole("button", { name: "Watch", exact: true })).toHaveAttribute("aria-pressed", "true");
    expect(await visiblePanels(page)).toEqual([]);
  });

  test("opening another tool ends the previous tool's input mode", async ({ page }) => {
    await startGame(page);
    await openTool(page, "Paths");
    await page.getByRole("button", { name: "Build paths" }).click();
    await openTool(page, "Landscape");
    await expect(page.locator("[data-paths] p[role='status']")).toHaveText("Path selection cancelled.");
    await openTool(page, "Paths");

    await openTool(page, "Coaster");
    await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
    await page.getByRole("button", { name: "Place station" }).click();
    await moveOverCanvas(page, await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 })));
    await expect(page.locator("canvas")).toHaveAttribute("data-station-preview", /^50,50,/);
    await openTool(page, "Facilities");
    await expect(page.locator("canvas")).toHaveAttribute("data-station-preview", "");
  });

  test("toolbar selection activates each tool's default map action", async ({ page }) => {
    test.setTimeout(120_000);
    await startGame(page);
    await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
    await openTool(page, "Paths");
    const pathStatus = page.getByRole("region", { name: "Paths" }).getByRole("status");
    await expect(pathStatus).toHaveText("Drag between tiles to build a path.");

    const pathPoints = await page.evaluate(() => [
      window.gameTest.project({ x: 48.5, y: 51.5, z: 2 }),
      window.gameTest.project({ x: 48.5, y: 47.5, z: 2 }),
    ]);
    await moveOverCanvas(page, pathPoints[0]);
    await expect.poll(() => page.evaluate(() => window.gameTest.pathPreview())).toHaveLength(1);
    const pathStart = await canvasToPage(page, pathPoints[0]);
    await page.mouse.move(pathStart.x, pathStart.y);
    await page.mouse.down();
    const pathEnd = await canvasToPage(page, pathPoints[1]);
    await page.mouse.move(pathEnd.x, pathEnd.y, { steps: 8 });
    await page.mouse.up();
    await expect(pathStatus).toHaveText("Path built.");
    await expect.poll(() => page.evaluate(() => window.gameTest.paths().length)).toBeGreaterThan(0);

    await openTool(page, "Landscape");
    await expect(page.getByRole("region", { name: "Landscaping" }).locator("p[role='status']")).toHaveText("Raise tiles active. Drag over the park to apply it.");
    const landscapePoint = await page.evaluate(() => window.gameTest.project({ x: 47.5, y: 49.5, z: 2 }));
    const landscapeTarget = await canvasToPage(page, landscapePoint);
    await page.mouse.click(landscapeTarget.x, landscapeTarget.y);
    const landscapeStatus = page.locator("[data-landscaping] p[role='status']");
    await expect(landscapeStatus).not.toHaveText("Raise tiles active. Drag over the park to apply it.");

    await openTool(page, "Scenery");
    const sceneryPoint = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 49.5, z: 2 }));
    const sceneryTarget = await canvasToPage(page, sceneryPoint);
    await page.mouse.click(sceneryTarget.x, sceneryTarget.y);
    await expect(page.getByRole("region", { name: "Scenery" }).getByRole("status")).not.toBeEmpty();

    await openTool(page, "Facilities");
    await expect(page.getByRole("region", { name: "Facilities" }).getByRole("status")).toHaveText("Choose a tile for the facility.");
    const facilityPoint = await page.evaluate(() => window.gameTest.project({ x: 53.5, y: 51.5, z: 2 }));
    const facilityTarget = await canvasToPage(page, facilityPoint);
    await page.mouse.click(facilityTarget.x, facilityTarget.y);
    await expect(page.getByRole("region", { name: "Facilities" }).getByRole("status")).not.toHaveText("Choose a tile for the facility.");
  });

  test("ignores coaster keys unless the coaster tool is open", async ({ page }) => {
    await startGame(page);
    await page.locator("canvas").focus();
    await page.keyboard.press("Delete");
    await openTool(page, "Coaster");
    // The checkbox shows once a station is placed, but it already holds the delete mode.
    const deleteMode = page.locator("[data-delete-mode]");
    await expect(deleteMode).not.toBeChecked();
    await page.locator("canvas").focus();
    await page.keyboard.press("Delete");
    await expect(deleteMode).toBeChecked();
  });

  test("resizes the world with the window", async ({ page }) => {
    await startGame(page);
    await page.setViewportSize({ width: 1000, height: 600 });
    await expect.poll(() => canvasSize(page)).toEqual({ width: 1000, height: 600 - barHeight });
  });
});

test.describe("high pixel density", () => {
  test.use({ viewport: { width: 1000, height: 600 }, deviceScaleFactor: 2 });

  test("draws the world at device resolution and the same size on screen", async ({ page }) => {
    await startGame(page);
    expect(await canvasSize(page)).toEqual({ width: 2000, height: 2 * (600 - barHeight) });
    await page.evaluate(() => window.gameTest.setCamera({ centerX: 240, centerY: 250, zoom: 1, rotation: 0 }));
    const step = await page.evaluate(() => {
      const origin = window.gameTest.project({ x: 240, y: 250, z: 0 });
      const east = window.gameTest.project({ x: 241, y: 250, z: 0 });
      return { x: east.x - origin.x, y: east.y - origin.y };
    });
    // One tile east is 32 by 16 CSS pixels at zoom 1, so 64 by 32 backing-store pixels at twice the density.
    expect(step).toEqual({ x: 64, y: 32 });
  });
});
