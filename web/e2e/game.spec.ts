import { expect, test } from "@playwright/test";

test("built game draws agents and advances simulation ticks", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible();
  await expect.poll(async () => canvas.getAttribute("data-agent-count")).toBe("12");

  const tickCount = page.getByTestId("tick-count");
  await expect.poll(async () => Number(await tickCount.textContent())).toBeGreaterThan(0);
  await expect.poll(async () => canvas.getAttribute("data-drawn-agents")).toBe("12");
  const visibleAgentPixels = await canvas.evaluate((element) => {
    const gameCanvas = element as HTMLCanvasElement;
    const context = gameCanvas.getContext("2d");
    if (!context) throw new Error("Canvas 2D context is unavailable");
    const pixels = context.getImageData(0, 0, gameCanvas.width, gameCanvas.height).data;
    let count = 0;
    for (let index = 0; index < pixels.length; index += 4) {
      if (pixels[index] === 112 && pixels[index + 1] === 47 && pixels[index + 2] === 85) count += 1;
    }
    return count;
  });
  expect(visibleAgentPixels).toBeGreaterThan(0);
});
