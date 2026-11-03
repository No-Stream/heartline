import { expect, test } from "@playwright/test";

test("built game draws agents and advances simulation ticks", async ({ page }) => {
  await page.goto("/?test=1");
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible();
  await expect.poll(async () => canvas.getAttribute("data-agent-count")).toBe("12");

  const tickCount = page.getByTestId("tick-count");
  await expect.poll(async () => Number(await tickCount.textContent())).toBeGreaterThan(0);
  await expect.poll(async () => canvas.getAttribute("data-drawn-agents")).toBe("12");
  expect(await canvas.evaluate((element) => !!(element as HTMLCanvasElement).getContext("webgl2"))).toBe(true);
  const visibleAgent = await page.evaluate(() => {
    const canvas = document.querySelector("canvas")!;
    for (let y = 4; y < canvas.height; y += 8) {
      for (let x = 4; x < canvas.width; x += 8) {
        const picked = window.gameTest.pick(x, y);
        if (picked?.kind === "sprite" && picked.id.startsWith("agent-")) return picked.id;
      }
    }
    return null;
  });
  expect(visibleAgent).toMatch(/^agent-\d+$/);
});
