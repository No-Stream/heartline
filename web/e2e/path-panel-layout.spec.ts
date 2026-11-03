import { expect, test } from "@playwright/test";
import { moveOverCanvas, openTool } from "./game-page";

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
