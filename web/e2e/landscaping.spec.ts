import { expect, test } from "@playwright/test";

test.use({
  viewport: { width: 1100, height: 1200 },
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
});

test("sculpts a hill, carves a tunnel and builds an underground path", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 150.5, centerY: 150.5, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  const landscaping = page.getByRole("region", { name: "Landscaping" });
  await landscaping.locator('[data-landscape-tool="flatten"]').click();
  await page.getByLabel("Brush size").selectOption("3");
  await page.getByLabel("Flatten height").fill("16");
  await landscaping.locator("[data-landscape-apply]").click();
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  const center = await worldPoint(page, 150, 150, 2);
  await clickCanvasPoint(page, center);
  await expect(landscaping.locator(".landscaping__status")).toHaveText("Terrain updated", { timeout: 10_000 });
  await landscaping.locator("[data-landscape-cancel]").click();
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const hill = await page.evaluate(() => {
    const point = window.gameTest.project({ x: 150.5, y: 150.5, z: 4 });
    return window.gameTest.pick(point.x, point.y);
  });
  expect(hill).toMatchObject({ kind: "terrain", x: 150, y: 150, height: 16 });
  await expect(page.locator("canvas")).toHaveScreenshot("landscaping-surface.png", {
    animations: "disabled",
    threshold: 0.1,
    maxDiffPixelRatio: 0.001,
  });

  await page.getByLabel("Brush size").selectOption("1");
  await page.getByLabel("Carve bottom").fill("4");
  await page.getByLabel("Carve ceiling").fill("10");
  for (const x of [149, 150, 151, 152, 153, 154, 155]) {
    const height = x === 152 ? 3 : x < 153 ? 4 : 2;
    const point = await worldPoint(page, x, 150, height);
    const pick = await page.evaluate((position) => window.gameTest.pick(position.x, position.y), point);
    expect(pick).toMatchObject({ kind: "terrain", x, y: 150 });
    await landscaping.locator('[data-landscape-tool="carve"]').click();
    await landscaping.locator("[data-landscape-apply]").click();
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
    await clickCanvasPoint(page, point);
    await expect(landscaping.locator(".landscaping__status")).toHaveText("Terrain updated", { timeout: 10_000 });
  }
  const tunnelSpans = await page.evaluate(() => window.gameTest.carvedSpans()
    .filter(([x, y]) => y === 150 && x >= 149 && x <= 155));
  expect(tunnelSpans).toEqual([
    [149, 150, 4, 10],
    [150, 150, 4, 10],
    [151, 150, 4, 10],
    [152, 150, 4, 10],
    [153, 150, 4, 8],
    [154, 150, 4, 8],
    [155, 150, 4, 8],
  ]);

  await landscaping.getByLabel("Underground cut level").fill("7");
  await landscaping.locator("[data-underground-toggle]").click();
  await expect(landscaping.locator("[data-underground-toggle]")).toHaveText("Show surface");
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await expect.poll(() => undergroundFloorPicks(page), { timeout: 15_000 }).toEqual(
    [149, 150, 151, 152, 153, 154, 155].map((x) => ["carved-floor", x, 150, 4]),
  );

  await page.getByLabel("Path type").selectOption("normal");
  await page.getByRole("button", { name: "Build paths" }).click();
  const tunnelPathPoints = await Promise.all([149, 155].map((x) => worldPoint(page, x, 150, 1)));
  await dragCanvasPoints(page, tunnelPathPoints);
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  await expect.poll(() => page.evaluate(() => window.gameTest.paths()
    .filter(({ x, y, z }) => y === 150 && x >= 149 && x <= 155 && z === 4).length)).toBe(7);
  await page.getByRole("region", { name: "Paths" }).getByRole("button", { name: "Cancel" }).click();
  await landscaping.locator("[data-landscape-cancel]").click();
  await expect(page.locator("canvas")).toHaveScreenshot("landscaping-underground.png", {
    animations: "disabled",
    threshold: 0.1,
    maxDiffPixelRatio: 0.001,
  });
});

async function worldPoint(
  page: import("@playwright/test").Page,
  x: number,
  y: number,
  height: number,
): Promise<{ x: number; y: number }> {
  return page.evaluate(({ tileX, tileY, tileHeight }) => window.gameTest.project({
    x: tileX + 0.5,
    y: tileY + 0.5,
    z: tileHeight,
  }), { tileX: x, tileY: y, tileHeight: height });
}

async function clickCanvasPoint(page: import("@playwright/test").Page, point: { x: number; y: number }): Promise<void> {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  await page.mouse.click(bounds.x + point.x * bounds.width / 800, bounds.y + point.y * bounds.height / 500);
}

async function dragCanvasPoints(page: import("@playwright/test").Page, points: readonly { x: number; y: number }[]): Promise<void> {
  if (points.length < 2) throw new Error("A drag needs at least two points");
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  const screen = (point: { x: number; y: number }): { x: number; y: number } => ({
    x: bounds.x + point.x * bounds.width / 800,
    y: bounds.y + point.y * bounds.height / 500,
  });
  const first = screen(points[0]!);
  await page.mouse.move(first.x, first.y);
  await page.mouse.down();
  for (const point of points.slice(1)) {
    const next = screen(point);
    await page.mouse.move(next.x, next.y, { steps: 4 });
  }
  await page.mouse.up();
}

async function undergroundFloorPicks(page: import("@playwright/test").Page): Promise<unknown[]> {
  return page.evaluate(() => [149, 150, 151, 152, 153, 154, 155].map((x) => {
    const point = window.gameTest.project({ x: x + 0.5, y: 150.5, z: 1 });
    const pick = window.gameTest.pick(point.x, point.y);
    return pick ? [pick.kind, pick.x, pick.y, pick.height] : null;
  }));
}
