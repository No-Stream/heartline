import { expect, test, type Locator } from "@playwright/test";
import { clickCanvas, moveOverCanvas, openTool } from "./game-page";

function heightValue(text: string, unit: "ft" | "m"): number {
  const match = text.match(/(-?\d+(?:\.\d+)?)\s*(ft|m)\b/);
  if (!match || match[2] !== unit) throw new Error(`Expected a height in ${unit}, received: ${text}`);
  return Number(match[1]);
}

async function readHeight(locator: Locator, unit: "ft" | "m"): Promise<number> {
  await expect(locator).toContainText(new RegExp(`\\d+(?:\\.\\d+)?\\s*${unit}\\b`));
  const text = await locator.textContent();
  if (text === null) throw new Error("Height display has no text");
  return heightValue(text, unit);
}

test("shows coaster height markers in the selected units and saves the preference", async ({ page }) => {
  test.setTimeout(90_000);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const units = page.getByRole("button", { name: "Units", exact: true });
  await expect(units).toHaveAttribute("data-units", "imperial");

  await openTool(page, "Coaster");
  const stationHeight = page.getByLabel("Station height above ground (feet)");
  await expect(stationHeight).toHaveValue("39");
  await expect(stationHeight).toHaveAttribute("step", "7");
  await page.getByRole("button", { name: "Raise the station" }).click();
  await expect(stationHeight).toHaveValue("46");
  await page.getByRole("button", { name: "Lower the station" }).click();
  await expect(stationHeight).toHaveValue("39");
  await units.click();
  await expect(page.getByLabel("Station height above ground (metres)")).toHaveValue("12");
  await units.click();
  await expect(stationHeight).toHaveValue("39");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.getByRole("button", { name: "Place station" }).click();
  const station = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, station);
  await clickCanvas(page, station);
  const direction = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, direction);
  await clickCanvas(page, direction);

  const track = page.locator("[data-track] li");
  await expect(track).toHaveCount(1);
  const palette = page.getByRole("radiogroup", { name: "Next piece" });
  for (const [index, piece] of ["LevelToGentleUp", "GentleUp", "GentleUp"].entries()) {
    await palette.locator(`[data-piece="${piece}"]`).click();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
  }

  const worldTrackHeight = page.locator("[data-height-markers] [data-track-height]");
  const worldStationHeight = page.locator("[data-height-markers] [data-station-height]");
  const panelTrackHeight = page.locator("[data-panel-track-height]");
  const panelStationHeight = page.locator("[data-panel-station-height]");
  const heightDisplays = [worldTrackHeight, worldStationHeight, panelTrackHeight, panelStationHeight];
  await expect(page.locator("[data-height-markers]")).toBeVisible();
  const imperialValues = await Promise.all(heightDisplays.map((display) => readHeight(display, "ft")));
  expect(imperialValues.every(Number.isInteger)).toBe(true);
  await page.evaluate(() => window.gameTest.setTimeOfDay(0.5));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await expect(page.locator("[data-height-markers]")).toBeVisible();
  await page.screenshot({ path: "/tmp/task-82-height-markers-light-day.png", fullPage: true });

  await page.getByRole("button", { name: "Dark interface" }).click();
  await page.evaluate(() => window.gameTest.setTimeOfDay(0.9));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await expect(page.locator("[data-height-markers]")).toBeVisible();
  await page.screenshot({ path: "/tmp/task-82-height-markers-dark-night.png", fullPage: true });

  await units.click();
  const metricValues = await Promise.all(heightDisplays.map((display) => readHeight(display, "m")));
  for (const [index, feet] of imperialValues.entries()) {
    expect(metricValues[index]).toBeCloseTo(feet * 0.3048, 0);
  }

  await page.reload();
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await expect(page.getByRole("button", { name: "Units", exact: true })).toHaveAttribute("data-units", "metric");
});

test("keeps the units setting usable when browser storage is unavailable", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      get() { throw new DOMException("Storage is unavailable", "SecurityError"); },
    });
  });
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const units = page.getByRole("button", { name: "Units", exact: true });
  await expect(units).toHaveAttribute("data-units", "imperial");
  await units.click();
  await expect(units).toHaveAttribute("data-units", "metric");
  expect(pageErrors).toEqual([]);
});
