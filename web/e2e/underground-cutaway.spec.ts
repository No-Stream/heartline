import { expect, test, type Page } from "@playwright/test";
import { clickTile, moveOverCanvas, openTool, placeStation, selectUnits, startPark } from "./player/driver";

test.use({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });

const ROUTE = [
  "LevelToGentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUpToLevel",
  "Straight", "OneTileStraight", "OneTileStraight", "OffsetLeft", "TurnLargeEntry", "TurnLargeExit",
  "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDownToLevel", "Straight",
];

test("the cutaway window shows coaster track and tunnels being built underground @slow", async ({ page }) => {
  test.setTimeout(240_000);
  await startPark(page, { park: "new", seed: 2027, speed: 1 });
  await raisePlateau(page);
  await page.getByLabel("Underground view style").selectOption("cutaway");
  await expect(page.getByRole("region", { name: "Landscaping" }).locator("[data-underground-hint]")).toContainText("Buried track shows through the ground");
  await buildRouteIntoTheGround(page);
  await page.evaluate(() => { window.gameTest.setTimeOfDay(0.45); window.gameTest.setCamera({ centerX: 57, centerY: 63, zoom: 1.5, rotation: 0 }); });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  expect(await page.evaluate(() => window.gameTest.cutaway())).toBeNull();
  const buriedTrackPicks = await constructionPicksAt(page, await page.evaluate(() => window.gameTest.project({ x: 61.5, y: 66.5, z: 1 })));
  expect(buriedTrackPicks, "buried track is hidden by the ground in the surface view").toBe(0);

  await page.keyboard.press("u");
  await expect.poll(() => page.evaluate(() => window.gameTest.cutaway())).not.toBeNull();
  const cutaway = (await page.evaluate(() => window.gameTest.cutaway()))!;
  expect(await constructionPicksAt(page, cutaway), "the window around the track end shows the track").toBeGreaterThan(0);
  const insidePick = await page.evaluate(({ x, y }) => window.gameTest.pickSurface(x, y + 30), cutaway);
  expect(insidePick?.height, "the window picks the cut ground, not the surface").toBeLessThanOrEqual(cutaway.level);
  const outsidePick = await page.evaluate(({ x, y, radius }) => window.gameTest.pickSurface(x + radius + 40, y), cutaway);
  expect(outsidePick).toMatchObject({ kind: "terrain" });
  expect(outsidePick!.height).toBeGreaterThan(cutaway.level);
  await expect(page.locator("canvas")).toHaveScreenshot("underground-cutaway-coaster.png", { animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001 });
  await page.getByRole("button", { name: "Dark interface", exact: true }).click();
  await page.evaluate(() => window.gameTest.setTimeOfDay(0.95));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await expect(page.locator("canvas"), "the window's edge and the buried track stay readable at night in the dark theme")
    .toHaveScreenshot("underground-cutaway-coaster-dark-night.png", { animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001 });

  const tunnel = await page.evaluate(() => window.gameTest.carvedSpans().filter(([x, y]) => y === 66 && x >= 60 && x <= 64)[0]);
  expect(tunnel, "the coaster carved a tunnel").toBeDefined();
  await openTool(page, "Landscape");
  await page.getByLabel("Underground cut level").fill(String(tunnel[2] + 2));
  await openTool(page, "Paths");
  await page.getByRole("button", { name: "Build paths" }).click();
  const tunnelFloor = await page.evaluate((span) => window.gameTest.project({ x: span[0] + 0.5, y: span[1] + 0.5, z: span[2] / 4 }), tunnel);
  await moveOverCanvas(page, tunnelFloor);
  await expect.poll(async () => {
    const shown = await page.evaluate(() => window.gameTest.cutaway());
    return shown && Math.hypot(shown.x - tunnelFloor.x, shown.y - tunnelFloor.y) < 2 && shown.level;
  }, { message: "the window follows the pointer at the chosen cut level" }).toBe(tunnel[2] + 2);
  expect(await page.evaluate(({ x, y }) => window.gameTest.pickSurface(x, y), tunnelFloor))
    .toMatchObject({ kind: "carved-floor", x: tunnel[0], y: tunnel[1], height: tunnel[2] });

  await page.keyboard.press("u");
  await expect.poll(() => page.evaluate(() => window.gameTest.cutaway())).toBeNull();
});

async function raisePlateau(page: Page): Promise<void> {
  await openTool(page, "Landscape");
  await selectUnits(page, "metric");
  const landscaping = page.getByRole("region", { name: "Landscaping" });
  await landscaping.getByRole("button", { name: "Flatten", exact: true }).click();
  await page.getByLabel("Brush size").selectOption("9");
  await page.getByLabel("Flatten height").fill("20");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 70.5, centerY: 67.5, zoom: 1.5, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await landscaping.getByRole("button", { name: "Apply tool" }).click();
  await clickTile(page, { x: 70, y: 67, z: 2 });
  await expect(landscaping.locator(".landscaping__status")).toHaveText("Terrain updated");
  await landscaping.getByRole("button", { name: "Cancel tool" }).click();
}

/** Climbs from a station, turns and dives under the plateau's foot, leaving the track end below ground. */
async function buildRouteIntoTheGround(page: Page): Promise<void> {
  await placeStation(page, { model: "hybrid", height: 4, tile: { x: 50, y: 50, z: 2 }, direction: { x: 52, y: 50, z: 2 } });
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track] li");
  for (const [index, piece] of ROUTE.entries()) {
    if (index === 0 || index === 8) await page.getByRole("button", { name: /Chain lift/ }).click();
    await choices.locator(`[data-piece="${piece}"]`).click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
  }
  await expect(page.locator("[data-track-height]")).toContainText("−");
}

async function constructionPicksAt(page: Page, center: { x: number; y: number }): Promise<number> {
  return page.evaluate(({ x, y }) => window.gameTest.pickRegion(Math.round(x) - 40, Math.round(y) - 40, 80, 80)
    .filter((pick) => pick && "id" in pick && pick.id.startsWith("construction-")).length, center);
}
