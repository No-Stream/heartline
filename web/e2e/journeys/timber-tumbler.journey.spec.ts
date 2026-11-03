import { expect, test } from "@playwright/test";
import { layPath, placeFacility, placePiece, placeStation, startPark } from "../player/driver";

test("Timber Tumbler player journey @journey", async ({ page }) => {
  test.setTimeout(240_000);
  await startPark(page, { park: "new", seed: 2026, speed: 16 });
  await layPath(page, { x: 218, y: 251, z: 2 }, { x: 218, y: 256, z: 2 });
  await placeFacility(page, "Food stall", { x: 216, y: 252, z: 2 });
  await placeFacility(page, "Drink stall", { x: 216, y: 254, z: 2 });
  await placeFacility(page, "Restroom", { x: 216, y: 256, z: 2 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await page.getByRole("radio", { name: "Begin chain lift climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  await page.getByRole("radio", { name: "Chain lift", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(3);
  // TASK-67, TASK-68 and TASK-69 add the chain toggle, climbing turns and 60-degree slopes.
  test.fail();
  await expect(page.getByRole("button", { name: "Turn left", exact: true }), "no climbing turn offered while the chain lift climbs").toBeEnabled();
  await page.getByRole("button", { name: "Turn left", exact: true }).click();
});
