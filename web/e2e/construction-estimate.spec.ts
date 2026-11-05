import { expect, test } from "@playwright/test";
import { clickCanvas, moveOverCanvas, openTool, selectUnits } from "./game-page";

test("shows a changing speed and force estimate on an unfinished coaster @fast", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/?park=new&seed=2026&test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").fill("4");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }

  const chain = page.getByRole("button", { name: "Chain lift", exact: true });
  await chain.click();
  await expect(chain).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("radio", { name: "Begin gentle climb", exact: true }).click();
  const placePiece = page.getByRole("button", { name: "Place piece" });
  const track = page.locator("[data-track] li");
  await placePiece.click();
  await expect.poll(() => track.count()).toBeGreaterThan(1);

  const estimate = page.locator("[data-construction-estimate]");
  const overlayMetric = page.getByLabel("Track overlay");
  await expect(estimate).toBeVisible();
  await expect(estimate).toContainText("Approximate construction estimate");
  await expect(estimate).toContainText("maximum speed");
  await expect(estimate).toContainText("Vertical");
  await expect(estimate).toContainText("Maximum lateral");
  await expect(estimate).toContainText("Maximum longitudinal");
  const firstEstimate = await estimate.textContent();
  await expect(overlayMetric).toHaveValue("speed");
  await expect(page.locator("[data-legend]")).toBeVisible();

  await page.getByRole("radio", { name: "Gentle climb", exact: true }).click();
  await placePiece.click();
  await expect.poll(() => track.count()).toBeGreaterThan(2);
  await expect.poll(() => estimate.textContent()).not.toBe(firstEstimate);

  await overlayMetric.selectOption("verticalG");
  await expect(overlayMetric).toHaveValue("verticalG");
  await expect(page.locator("[data-legend]")).toBeVisible();
  await expect(page.locator("canvas")).toHaveScreenshot("construction-force-estimate.png", {
    animations: "disabled",
    threshold: 0.1,
    maxDiffPixelRatio: 0.001,
  });
});
