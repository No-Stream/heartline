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
  await page.evaluate(() => {
    const guest = window.gameTest.guests()[0];
    window.gameTest.setCamera({ centerX: guest.x, centerY: guest.y, zoom: 2, rotation: 0 });
  });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const visibleAgent = await page.evaluate(() => {
    const canvas = document.querySelector("canvas")!;
    for (let y = 4; y < canvas.height; y += 8) {
      for (let x = 4; x < canvas.width; x += 8) {
        const picked = window.gameTest.pick(x, y);
        if (picked?.kind === "sprite" && picked.id.startsWith("guest-")) return picked.id;
      }
    }
    return null;
  });
  expect(visibleAgent).toMatch(/^guest-\d+$/);
});


test("walking guests change their rendered positions as simulation ticks advance", async ({ page }) => {
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => window.gameTest?.guests().length ?? 0)).toBeGreaterThan(0);
  const initial = await page.evaluate(() => window.gameTest.guests().map(({ id, x, y }) => ({ id, x, y })));
  await expect.poll(() => page.evaluate((before) => {
    const positions = new Map(before.map((guest) => [guest.id, guest]));
    return window.gameTest.guests().some((guest) => {
      const old = positions.get(guest.id);
      return old !== undefined && (guest.x !== old.x || guest.y !== old.y);
    });
  }, initial)).toBe(true);
});
