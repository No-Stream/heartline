import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { openTool, showBuilderTab } from "./game-page";
import { parseReplaySession } from "../src/session-replay";
import { COASTER_COLORS } from "../src/coaster-colors";

type Rgb = [number, number, number];

const rgb = (hex: string): Rgb => [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16)) as Rgb;

/**
 * Canvas pixels that the built coaster's track covers, read from the drawing buffer the test mode keeps. The replayed
 * park keeps running, so the clock is set back to noon first, where the grade leaves every colour as it is.
 */
async function trackPixels(page: Page): Promise<Rgb[]> {
  return page.evaluate(async () => {
    window.gameTest.setTimeOfDay(0.5);
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    const canvas = document.querySelector("canvas")!;
    const gl = canvas.getContext("webgl2")!;
    const pixels: [number, number, number][] = [];
    for (let y = 4; y < canvas.height; y += 5) for (let x = 4; x < canvas.width * 0.65; x += 5) {
      const pick = window.gameTest.pick(x, y);
      if (!pick || !("id" in pick) || !pick.id.startsWith("construction-track-")) continue;
      const color = new Uint8Array(4);
      gl.readPixels(x, canvas.height - 1 - y, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, color);
      pixels.push([color[0], color[1], color[2]]);
    }
    return pixels;
  });
}

const near = (pixel: Rgb, target: Rgb): boolean => pixel.every((channel, index) => Math.abs(channel - target[index]) <= 16);

test("the construction panel repaints a built coaster's track, supports and trains", async ({ page }) => {
  test.setTimeout(120_000);
  const session = parseReplaySession(JSON.parse(readFileSync(new URL("./sessions/timber-tumbler-built.json", import.meta.url), "utf8")) as unknown);
  const params = new URLSearchParams({ test: "1", manualBlockAnalysis: "1", park: session.park, seed: session.seed, agents: String(session.guestCount) });
  await page.goto(`/?${params}`);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate((recordedSession) => window.gameTest.replaySession(recordedSession), session);
  await openTool(page, "Coaster");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 236, centerY: 246, zoom: 1.1, rotation: 0 }));
  await showBuilderTab(page, "Colours");

  const swatch = (part: string, color: string) => page.getByRole("radiogroup", { name: `${part} colour` }).getByRole("radio", { name: color, exact: true });
  await expect(swatch("Track", "Coral")).toHaveAttribute("aria-checked", "true");
  await expect(swatch("Supports", "Timber")).toHaveAttribute("aria-checked", "true");
  await expect(swatch("Trains", "Sage")).toHaveAttribute("aria-checked", "true");
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const before = await trackPixels(page);
  expect(before.length).toBeGreaterThan(50);
  expect(before.some((pixel) => near(pixel, rgb(COASTER_COLORS.coral.hex)))).toBe(true);
  expect(before.some((pixel) => near(pixel, rgb(COASTER_COLORS.teal.hex)))).toBe(false);

  await swatch("Track", "Teal").click();
  await swatch("Supports", "Cream").click();
  await swatch("Trains", "Mustard").click();
  await expect(swatch("Track", "Teal")).toHaveAttribute("aria-checked", "true");
  await expect(swatch("Track", "Coral")).toHaveAttribute("aria-checked", "false");
  await expect(page.locator('[data-coaster-color-part="supports"]')).toHaveAttribute("data-selected", "cream");
  await expect(page.locator('[data-coaster-color-part="trains"]')).toHaveAttribute("data-selected", "mustard");

  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const after = await trackPixels(page);
  expect(after.some((pixel) => near(pixel, rgb(COASTER_COLORS.teal.hex)))).toBe(true);
  expect(after.some((pixel) => near(pixel, rgb(COASTER_COLORS.coral.hex)))).toBe(false);

  // The colours belong to the simulation's coaster, so they come back after leaving the coaster tool.
  await openTool(page, "Watch");
  await openTool(page, "Coaster");
  await expect(swatch("Track", "Teal")).toHaveAttribute("aria-checked", "true");
});
