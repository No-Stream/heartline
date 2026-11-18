import { expect, test } from "@playwright/test";
import { clickCanvas, moveOverCanvas, openTool, selectUnits } from "../game-page";
import { browserLaunchOptions } from "../fixtures/browser-launch-options";
import { gotoGame } from "../player/driver";

test.use({ launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]) });

test("raises a slow Velocity Viper launch through the builder and completes the test @slow @journey", async ({ page }) => {
  test.setTimeout(240_000);
  await gotoGame(page, "/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.getByRole("combobox", { name: "Coaster model" }).selectOption("model:velocity_viper");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.getByLabel("Station height above ground (metres)").fill("12");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();

  const stationPosition = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, stationPosition);
  await clickCanvas(page, stationPosition);
  const east = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, east);
  await clickCanvas(page, east);

  const pieces = ["Short launch", "Small left turn", "Small left turn", "Short launch", "Straight", "Small left turn", "Small left turn"];
  for (const [index, piece] of pieces.entries()) {
    await page.getByRole("radio", { name: piece, exact: true }).click();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(page.locator("[data-track] li")).toHaveCount(index + 2);
  }

  const launchRows = page.getByRole("region", { name: "Launch positions" }).locator("li");
  await expect(launchRows).toHaveCount(2);
  const launchTargets = [
    page.getByLabel("Launch 2 target (km/h)"),
    page.getByLabel("Launch 5 target (km/h)"),
  ];
  const launchLimits = await Promise.all(launchTargets.map(async (input) => ({
    minimum: Number(await input.getAttribute("min")),
    maximum: Number(await input.getAttribute("max")),
  })));
  expect(launchLimits[0].minimum).toBeGreaterThan(0);
  expect(launchLimits[0].maximum).toBeGreaterThan(launchLimits[0].minimum);
  expect(launchLimits[1].maximum).toBeGreaterThan(launchLimits[1].minimum);
  for (const [index, input] of launchTargets.entries()) {
    await input.fill(String(launchLimits[index].minimum));
    await input.press("Tab");
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await page.getByRole("button", { name: "Test coaster" }).click();
  await page.evaluate(() => window.gameTest.advanceTicks(10_000));
  await expect.poll(() => page.getByRole("region", { name: "Coaster construction" }).getByRole("status").textContent())
    .toBe("Train completed a lap.");
  const initialLaunchSpeeds = await page.getByRole("list", { name: "Launch exit speeds" }).locator("li").allTextContents();
  const initialFirstLaunchSpeed = Number(initialLaunchSpeeds[0].match(/exit speed: ([\d.]+)/)?.[1]);

  for (const input of launchTargets) {
    await input.fill("50");
    await input.press("Tab");
  }
  await page.getByRole("button", { name: "Test coaster" }).click();
  await page.evaluate(() => window.gameTest.advanceTicks(10_000));
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status"))
    .toHaveText("Train completed a lap.", { timeout: 150_000 });
  const increasedLaunchSpeeds = await page.getByRole("list", { name: "Launch exit speeds" }).locator("li").allTextContents();
  const increasedFirstLaunchSpeed = Number(increasedLaunchSpeeds[0].match(/exit speed: ([\d.]+)/)?.[1]);
  expect(increasedFirstLaunchSpeed).toBeGreaterThan(initialFirstLaunchSpeed);
});
