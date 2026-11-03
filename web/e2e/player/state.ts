import type { Page } from "@playwright/test";

export async function revenue(page: Page): Promise<number> {
  const value = await page.locator("[data-revenue]").textContent();
  if (value === null || !/^\d+$/.test(value.trim())) throw new Error(`Invalid revenue: ${value}`);
  return Number(value);
}

export async function tickCount(page: Page): Promise<number> {
  return page.evaluate(() => window.gameTest.tickCount());
}

export async function riders(page: Page): Promise<readonly { id: string; car: number; seat: number; x: number; y: number; z: number }[]> {
  return page.evaluate(() => window.gameTest.riders());
}

export async function queuedGuests(page: Page): Promise<readonly string[]> {
  return page.evaluate(() => {
    const queue = new Set(window.gameTest.paths().filter(({ kind }) => kind === 1).map(({ x, y }) => `${x},${y}`));
    return window.gameTest.guests().filter(({ x, y }) => queue.has(`${Math.floor(x)},${Math.floor(y)}`)).map(({ id }) => id);
  });
}
