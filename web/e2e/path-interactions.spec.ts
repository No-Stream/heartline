import { expect, test, type Page } from "@playwright/test";

test.use({
  viewport: { width: 1100, height: 1200 },
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
});

test("single-click placement works and a refused queue preview explains the obstruction", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 218, centerY: 254, zoom: 2, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  await page.getByLabel("Path type").selectOption("normal");
  await page.getByRole("button", { name: "Build paths" }).click();
  await clickTile(page, 217, 256, 2);
  await expect.poll(() => pathAt(page, 217, 256, 2)).toBe(true);
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");

  const pathsBeforeRefusedPreview = await page.evaluate(() => window.gameTest.paths().length);
  await page.getByLabel("Path type").selectOption("queue");
  await page.getByRole("button", { name: "Build paths" }).click();
  const point = await screenPoint(page, 218, 254, 2);
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  const status = page.getByRole("region", { name: "Paths" }).getByRole("status");
  await expect(status).toHaveText("Path route overlaps another path");
  await expect.poll(() => page.evaluate(() => window.gameTest.paths().length)).toBe(pathsBeforeRefusedPreview);
  await page.mouse.up();
  await expect(status).toHaveText("Path route overlaps another path");
  await expect.poll(() => page.evaluate(() => window.gameTest.paths().length)).toBe(pathsBeforeRefusedPreview);
});

async function pathAt(page: Page, x: number, y: number, z: number): Promise<boolean> {
  return page.evaluate(({ tileX, tileY, height }) => window.gameTest.paths()
    .some((path) => path.x === tileX && path.y === tileY && path.z === height * 4),
  { tileX: x, tileY: y, height: z });
}

async function clickTile(page: Page, x: number, y: number, z: number): Promise<void> {
  const point = await screenPoint(page, x, y, z);
  await page.mouse.click(point.x, point.y);
}

async function screenPoint(page: Page, x: number, y: number, z: number): Promise<{ x: number; y: number }> {
  const projected = await page.evaluate(({ tileX, tileY, height }) => window.gameTest.project({
    x: tileX + 0.5,
    y: tileY + 0.5,
    z: height,
  }), { tileX: x, tileY: y, height: z });
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  return {
    x: bounds.x + projected.x * bounds.width / 800,
    y: bounds.y + projected.y * bounds.height / 500,
  };
}
