import { expect, test } from "@playwright/test";

test("builds and runs a wooden twister", async ({ page }) => {
  test.setTimeout(240_000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.getByRole("combobox", { name: "Coaster model" }).selectOption("model:wooden_twister");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  for (const point of [{ x: 50.5, y: 50.5, z: 2 }, { x: 52.5, y: 50.5, z: 2 }]) {
    const position = await page.evaluate((worldPoint) => window.gameTest.project(worldPoint), point);
    await page.mouse.move(bounds.x + position.x * bounds.width / 800, bounds.y + position.y * bounds.height / 500);
    await page.mouse.click(bounds.x + position.x * bounds.width / 800, bounds.y + position.y * bounds.height / 500);
  }
  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track]");
  const pieces = [
    "LiftLevelToGentleUp", "Lift", "Lift", "Lift", "Lift", "Lift", "LiftGentleUpToLevel",
    "LiftFlat", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit",
    "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown",
    "GentleDownToLevel", "SteelLoop", "TurnLargeEntry", "TurnLargeExit",
  ];
  for (const [index, piece] of pieces.entries()) {
    await choices.locator(`[data-piece="${piece}"]`).click();
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await page.getByRole("button", { name: "Test coaster" }).click();
  await page.evaluate(() => window.gameTest.advanceTicks(10_000));
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status"))
    .toHaveText("Train completed a lap.", { timeout: 150_000 });
  await expect.poll(() => page.evaluate(() => window.gameTest.testingReport()?.samples.length ?? 0)).toBeGreaterThan(1);
  expect(errors).toEqual([]);
});
