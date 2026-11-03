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
  await expect(
    page.getByRole("button", { name: "Slope up", exact: true }),
    "the 60-degree climb transition is not offered from the 45-degree chain lift",
  ).toBeEnabled();
  await page.getByRole("button", { name: "Slope up", exact: true }).click();
  await expect(previewName).toHaveText("Begin 60-degree climb");
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(6);
  await expect(previewName).toHaveText("60-degree climb");
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(7);

  await page.getByRole("radio", { name: "End 60-degree climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(8);
  await page.keyboard.press("h");
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");
  await page.getByRole("radio", { name: "End steep climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(9);
  await page.getByRole("radio", { name: "End gentle climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(10);

  await page.getByRole("radio", { name: "Begin gentle descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(11);
  await page.getByRole("radio", { name: "Gentle descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(12);
  await page.getByRole("radio", { name: "Begin steep descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(13);
  await page.getByRole("radio", { name: "Steep descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(14);
  await page.getByRole("radio", { name: "Begin 60-degree descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(15);
  await page.getByRole("radio", { name: "60-degree descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(16);
  await page.getByRole("radio", { name: "End 60-degree descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(17);
  await page.getByRole("radio", { name: "End steep descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(18);
  await page.getByRole("radio", { name: "End gentle descent", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(19);

  await page.getByRole("radio", { name: "Bank left", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(20);
  await page.getByRole("radio", { name: "Standalone small left-banked left turn", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(21);
  await page.getByRole("radio", { name: "Standalone small left-banked left turn", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(22);
  await page.getByRole("radio", { name: "Level from left bank", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(23);
  await page.getByRole("radio", { name: "Small airtime hill", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(24);
});
