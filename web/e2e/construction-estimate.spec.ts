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
  await expect(overlayMetric).toHaveValue("limitCloseness");
  await expect(page.locator("[data-band-legend]")).toBeVisible();
  await expect(page.locator("[data-band-legend]")).toContainText("Near limit");
  await expect(page.locator("[data-legend]")).toBeHidden();

  await page.getByRole("radio", { name: "Gentle climb", exact: true }).click();
  await placePiece.click();
  await expect.poll(() => track.count()).toBeGreaterThan(2);
  await expect.poll(() => estimate.textContent()).not.toBe(firstEstimate);
  const topSpeed = page.locator('[data-force-label="topSpeed"]');
  await expect(topSpeed).toBeVisible();
  await expect(topSpeed).toHaveText(/^\d+ km\/h$/);

  const samples = await page.evaluate(() => window.gameTest.constructionForcePreview()!.samples);
  const hovered = samples[Math.floor(samples.length / 2)];
  await moveOverCanvas(page, await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), hovered));
  const readout = page.locator("[data-force-readout]");
  await expect(readout).toBeVisible();
  for (const value of ["Speed", "km/h", "Vertical", "Lateral", "Longitudinal"]) await expect(readout).toContainText(value);
  await expect(readout).toContainText(/Lateral\s*−?\d\.\d g · (fine|strong|too much|dangerous)/);
  await moveOverCanvas(page, { x: 4, y: 4 });
  await expect(readout).toBeHidden();
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await expect(page.locator("canvas")).toHaveScreenshot("construction-force-estimate.png", {
    animations: "disabled",
    threshold: 0.1,
    maxDiffPixelRatio: 0.001,
  });

  await overlayMetric.selectOption("verticalG");
  await expect(overlayMetric).toHaveValue("verticalG");
  await expect(page.locator("[data-legend]")).toBeVisible();
  await expect(page.locator("[data-band-legend]")).toBeHidden();
  await expect(topSpeed).toBeVisible();
  await page.getByRole("tab", { name: "Operate" }).click();
  const cars = page.locator("[data-cars-per-train]");
  const capacity = page.locator("[data-train-rider-capacity]");
  const initialCars = Number(await cars.textContent());
  const initialCapacity = await capacity.textContent();
  const initialSamples = await page.evaluate(() => window.gameTest.constructionForcePreview()!.samples);
  await page.getByRole("button", { name: "Remove one car per train" }).click();
  await expect(cars).toHaveText(String(initialCars - 1));
  await expect(capacity).not.toHaveText(initialCapacity!);
  await expect.poll(() => page.evaluate(() => window.gameTest.constructionForcePreview()!.samples)).not.toEqual(initialSamples);
  await page.getByRole("tab", { name: "Build" }).click();
  await expect(estimate).toContainText("Approximate construction estimate");
});

test("marks forces past the model's comfortable limits on a harsh drop @fast", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/?park=new&seed=2026&test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").fill("60");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await moveOverCanvas(page, position);
    await clickCanvas(page, position);
  }
  const track = page.locator("[data-track] li");
  const placePiece = page.getByRole("button", { name: "Place piece" });
  for (const name of ["Begin 60-degree descent", "Level out from 60-degree descent", "Small airtime hill"]) {
    const count = await track.count();
    await page.getByRole("radio", { name, exact: true }).click();
    await placePiece.click();
    await expect.poll(() => track.count()).toBeGreaterThan(count);
  }
  await expect(page.locator("[data-construction-estimate]")).toContainText("exceed this model's comfortable limits");
  await expect(page.getByLabel("Track overlay")).toHaveValue("limitCloseness");
  await page.evaluate(() => {
    const { samples, limits } = window.gameTest.constructionForcePreview()!;
    const closeness = (sample: (typeof samples)[number]): number => Math.max(
      sample.verticalG < 0 ? sample.verticalG / limits.minimumVerticalG : sample.verticalG / limits.maximumVerticalG,
      Math.abs(sample.lateralG) / limits.maximumLateralG, Math.abs(sample.longitudinalG) / limits.maximumLongitudinalG);
    const worst = samples.reduce((current, sample) => closeness(sample) > closeness(current) ? sample : current);
    window.gameTest.setCamera({ centerX: worst.x - worst.z, centerY: worst.y - worst.z, zoom: 1, rotation: 0 });
  });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const pastLimit = page.locator('[data-force-label][data-past-limit="true"]');
  await expect(pastLimit.first()).toBeVisible();
  await expect(pastLimit.first()).toHaveText(/g · (too much|dangerous) /);
  await expect(page.locator("[data-band-legend]")).toContainText("Past limit");
});
