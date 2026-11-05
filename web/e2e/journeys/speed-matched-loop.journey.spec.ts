import { expect, test } from "@playwright/test";
import { clickCanvas, moveOverCanvas, openTool, selectUnits } from "../game-page";

test("selects a loop sized for the estimated launch speed and previews its forces @journey", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/?park=new&seed=2026&test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.getByRole("combobox", { name: "Coaster model" }).selectOption("model:velocity_viper");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.getByLabel("Station height above ground (metres)").fill("12");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  await page.getByRole("radio", { name: "Long launch", exact: true }).click();
  await page.getByRole("button", { name: "Place piece" }).click();
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  const launchTarget = page.getByLabel("Launch 2 target (km/h)");
  await launchTarget.fill("108");
  await launchTarget.press("Tab");
  await expect(launchTarget).toHaveValue("108");

  await page.getByRole("radiogroup", { name: "Next piece" }).locator('[data-piece="Loop"]').click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", "LargeLoop");
  const preview = page.locator("[data-candidate-force-preview]");
  await expect(preview).toBeVisible();
  await expect(preview).toContainText(/entry speed/i);
  await expect(preview).toContainText(/vertical/i);
  await expect(preview).toContainText(/lateral/i);
  await expect(preview).toContainText(/longitudinal/i);
  await expect(preview).toContainText(/weak|good|great|too much/i);

  const sizes = page.getByRole("group", { name: "Element size" });
  const largeLoop = sizes.locator('[data-speed-match-variant="LargeLoop"]');
  const standardLoop = sizes.locator('[data-speed-match-variant="Loop"]');
  await expect(largeLoop).toHaveAttribute("aria-pressed", "true");
  const recommendedPreview = await preview.textContent();
  await standardLoop.click();
  await expect(standardLoop).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", "Loop");
  await expect.poll(() => preview.textContent()).not.toBe(recommendedPreview);
});
