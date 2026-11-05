import { expect, test } from "@playwright/test";
import { placePiece, placeStation, startPark } from "./player/driver";

test("builds an Immelmann from the construction palette", async ({ page }) => {
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 50, y: 50, z: 2 }, direction: { x: 52, y: 50, z: 2 } });

  const choice = page.locator('[data-element="element:immelmann-left"]');
  await expect(choice).toBeVisible();
  await expect(page.getByRole("heading", { name: "Modern inversions (heartlined)" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Classic inversions (not heartlined)" })).toBeVisible();
  await expect(page.locator('[data-heartlined-group="true"] [data-track-choice="HeartlineRoll"]')).toBeVisible();
  await expect(page.locator('[data-heartlined-group="false"] [data-track-choice="Loop"]')).toBeVisible();
  await expect(page.locator('[data-heartlined-group="false"] [data-element="element:immelmann-left"]')).toBeVisible();
  await choice.click();
  await expect(page.locator("[data-next] [data-preview-name]")).toHaveText("Left Immelmann");
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(3);
  await expect(page.locator("[data-track] li")).toContainText(["Station", "half loop", "half roll"], { ignoreCase: true });
});
