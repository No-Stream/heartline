import { expect, test } from "@playwright/test";
import { layPath, placeFacility, placePiece, placeStation, startPark } from "../player/driver";

test("Timber Tumbler player journey @journey", async ({ page }) => {
  test.setTimeout(240_000);
  const previewName = page.locator("[data-next] [data-preview-name]");
  await startPark(page, { park: "new", seed: 2026, speed: 16 });
  await layPath(page, { x: 218, y: 251, z: 2 }, { x: 218, y: 256, z: 2 });
  await placeFacility(page, "Food stall", { x: 216, y: 252, z: 2 });
  await placeFacility(page, "Drink stall", { x: 216, y: 254, z: 2 });
  await placeFacility(page, "Restroom", { x: 216, y: 256, z: 2 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await page.keyboard.press("h");
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "true");
  await page.getByRole("radio", { name: "Begin gentle climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  await page.getByRole("radio", { name: "Gentle climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Turn left", exact: true }), "no climbing turn offered while the chain lift climbs").toBeEnabled();
  await page.getByRole("button", { name: "Turn left", exact: true }).click();
  await expect(previewName).toHaveText("Climbing standalone small left turn");
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(4);

  await page.getByRole("button", { name: "Turn right", exact: true }).click();
  await expect(previewName).toHaveText("Gentle climb");
  await page.getByRole("button", { name: "Slope up", exact: true }).click();
  await expect(previewName).toHaveText("Begin steep climb");
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(5);

  await page.getByRole("radio", { name: "Steep climb", exact: true }).click();
  // TASK-69 adds the first 60-degree chain-lift transition after this 45-degree climb.
  test.fail(true, "TASK-69: 60-degree slope pieces are not implemented");
  await expect(
    page.getByRole("button", { name: "Slope up", exact: true }),
    "TASK-69: the 60-degree climb is not offered from the 45-degree chain lift",
  ).toBeEnabled();
});
