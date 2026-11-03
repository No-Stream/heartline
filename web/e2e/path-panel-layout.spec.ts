import { expect, test } from "@playwright/test";
import { canvasToPage, clickCanvas, moveOverCanvas, openTool } from "./game-page";

test("the path status does not move the canvas during a drag", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Paths");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 247, centerY: 253, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Path type").selectOption("queue");
  await page.getByRole("button", { name: "Build paths" }).click();

  const points = await page.evaluate(() => [
    window.gameTest.project({ x: 244.5, y: 255.5, z: 2 }),
    window.gameTest.project({ x: 244.5, y: 250.5, z: 2 }),
  ]);
  await page.evaluate(() => {
    (window as unknown as { pathCanvasTops: number[] }).pathCanvasTops = [];
    document.addEventListener("pointermove", (event) => {
      if (event.buttons === 1) (window as unknown as { pathCanvasTops: number[] }).pathCanvasTops.push(document.querySelector("canvas")!.getBoundingClientRect().top);
    });
  });
  await moveOverCanvas(page, points[0]);
  await page.mouse.down();
  await moveOverCanvas(page, points[1], { steps: 12 });
  await page.mouse.up();
  const tops = await page.evaluate(() => (window as unknown as { pathCanvasTops: number[] }).pathCanvasTops);
  expect(tops.length).toBeGreaterThan(1);
  expect(Math.max(...tops) - Math.min(...tops)).toBeLessThan(1);
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  await expect.poll(() => page.evaluate(() => window.gameTest.paths().filter(({ kind }) => kind === 1).map(({ x, y }) => [x, y]))).toEqual(
    [250, 251, 252, 253, 254, 255].map((y) => [244, y]),
  );
});

test("toolbar path drags preview and place reciprocal normal path edges at every rotation and zoom", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Paths");
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Drag between tiles to build a path.");

  for (const rotation of [0, 1, 2, 3] as const) {
    for (const zoom of [0.75, 1.5]) {
      await page.evaluate(({ rotation, zoom }) => window.gameTest.setCamera({ centerX: 247, centerY: 253, zoom, rotation }), { rotation, zoom });
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      const startTile = { x: 244, y: 255 };
      const routeTiles = Array.from({ length: 6 }, (_, index) => ({ x: startTile.x, y: startTile.y - index }));
      const [startPoint, endPoint] = await page.evaluate(() => [
        window.gameTest.project({ x: 244.5, y: 255.5, z: 2 }),
        window.gameTest.project({ x: 244.5, y: 250.5, z: 2 }),
      ]);
      const start = await canvasToPage(page, startPoint);
      const end = await canvasToPage(page, endPoint);
      const pickedEndpoints = await page.evaluate(([start, end]) => [
        window.gameTest.pickSurface(start.x, start.y),
        window.gameTest.pickSurface(end.x, end.y),
      ], [startPoint, endPoint]);
      expect(pickedEndpoints.map((tile) => tile && [tile.x, tile.y, tile.kind])).toEqual([
        [244, 255, "terrain"],
        [244, 250, "terrain"],
      ]);
      await page.mouse.move(start.x, start.y);
      await page.mouse.down();
      await page.mouse.move(end.x, end.y, { steps: 4 });
      await expect.poll(() => page.evaluate(() => window.gameTest.pathPreview())).toHaveLength(6);
      const preview = await page.evaluate(() => window.gameTest.pathPreview());
      const expectedRoute = routeTiles.map(({ x: tileX, y: tileY }) => [tileX, tileY]);
      expect(preview.map(({ x: tileX, y: tileY }) => [tileX, tileY])).toEqual(expectedRoute);
      await page.mouse.up();
      await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");

      await expect.poll(() => page.evaluate(({ x, y }) => window.gameTest.paths().filter((path) => path.x === x && path.y <= y && path.y >= y - 5).length, startTile)).toBeGreaterThanOrEqual(6);
      const placedPaths = await page.evaluate(({ x, y }) => window.gameTest.paths().filter((path) => path.x === x && path.y <= y && path.y >= y - 5), startTile);
      expect(placedPaths.map(({ x: tileX, y: tileY }) => [tileX, tileY]).sort((left, right) => left[1] - right[1])).toEqual([...expectedRoute].sort((left, right) => left[1] - right[1]));
      for (const path of placedPaths) {
        if (path.y < startTile.y) expect(path.edges & 4).toBeTruthy();
        if (path.y > startTile.y - 5) expect(path.edges & 1).toBeTruthy();
      }

      await page.getByRole("button", { name: "Remove paths" }).click();
      for (const path of placedPaths) {
        const point = await page.evaluate(({ x, y, z }) => window.gameTest.project({ x: x + 0.5, y: y + 0.5, z: z / 4 }), path);
        await clickCanvas(page, point);
      }
      await expect.poll(() => page.evaluate(() => window.gameTest.paths().filter(({ x, y }) => x === 244 && y <= 255 && y >= 250).length)).toBe(0);
      await page.getByRole("button", { name: "Build paths" }).click();
    }
  }
});
