import { expect, test } from "@playwright/test";

test("a frozen world remains available after the browser presents it", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const pixel = await page.evaluate(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    const canvas = document.querySelector("canvas")!;
    const gl = canvas.getContext("webgl2")!;
    const color = new Uint8Array(4);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, color);
    return [...color];
  });
  expect(pixel.slice(0, 3)).not.toEqual([0, 0, 0]);
});
