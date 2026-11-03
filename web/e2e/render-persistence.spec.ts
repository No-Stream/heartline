import { expect, test, type Page } from "@playwright/test";

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

test("a frozen world redraws after WebGL context restoration", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await waitForCanvasPresentation(page);
  const beforePath = await visiblePathPixel(page);
  const beforeScreenshot = await page.locator("canvas").screenshot();

  const contextLifecycle = await page.evaluate(async () => {
    const canvas = document.querySelector("canvas")!;
    const gl = canvas.getContext("webgl2")!;
    const loseContext = gl.getExtension("WEBGL_lose_context");
    if (!loseContext) throw new Error("WEBGL_lose_context is unavailable");
    const lost = new Promise<boolean>((resolve, reject) => {
      const timeout = window.setTimeout(() => reject(new Error("WebGL context loss event did not fire")), 5_000);
      canvas.addEventListener("webglcontextlost", (event) => {
        window.clearTimeout(timeout);
        resolve(event.defaultPrevented);
      }, { once: true });
    });
    loseContext.loseContext();
    const prevented = await lost;
    const lostContext = { prevented, ready: window.gameTest.ready, contextLost: gl.isContextLost() };
    await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
    const restored = new Promise<boolean>((resolve, reject) => {
      const timeout = window.setTimeout(() => reject(new Error("WebGL context restored event did not fire")), 5_000);
      canvas.addEventListener("webglcontextrestored", () => {
        window.clearTimeout(timeout);
        resolve(window.gameTest.ready);
      }, { once: true });
    });
    loseContext.restoreContext();
    const readyAtRestore = await restored;
    return { lostContext, restoredContext: { ready: readyAtRestore, contextLost: gl.isContextLost() } };
  });

  expect(contextLifecycle.lostContext.prevented).toBe(true);
  expect(contextLifecycle.lostContext.contextLost).toBe(true);
  expect(contextLifecycle.lostContext.ready).toBe(false);
  expect(contextLifecycle.restoredContext.contextLost).toBe(false);
  expect(contextLifecycle.restoredContext.ready).toBe(false);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready), { timeout: 5_000 }).toBe(true);
  await waitForCanvasPresentation(page);
  const afterPath = await visiblePathPixel(page);
  const afterScreenshot = await page.locator("canvas").screenshot();
  expect(afterPath).toEqual(beforePath);
  expect(afterScreenshot).toEqual(beforeScreenshot);
});

async function waitForCanvasPresentation(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
}

async function visiblePathPixel(page: Page): Promise<{ tile: [number, number]; pixel: [number, number]; rgba: number[] }> {
  return page.evaluate(() => {
    const canvas = document.querySelector("canvas")!;
    const gl = canvas.getContext("webgl2")!;
    for (const path of window.gameTest.paths()) {
      for (let heightOffset = 0; heightOffset <= 1; heightOffset += 0.125) {
        const position = window.gameTest.project({ x: path.x + 0.5, y: path.y + 0.5, z: path.z / 4 + heightOffset });
        if (position.x < 0 || position.y < 0 || position.x >= canvas.width || position.y >= canvas.height) continue;
        const selected = window.gameTest.pick(position.x, position.y);
        if (selected?.kind !== "path" || selected.x !== path.x || selected.y !== path.y) continue;
        const pixel = [Math.floor(position.x), Math.floor(position.y)] as [number, number];
        const color = new Uint8Array(4);
        gl.readPixels(pixel[0], canvas.height - 1 - pixel[1], 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, color);
        return { tile: [path.x, path.y] as [number, number], pixel, rgba: [...color] };
      }
    }
    throw new Error("No rendered path pixel could be sampled");
  });
}
