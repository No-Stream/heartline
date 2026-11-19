import { expect, test } from "@playwright/test";
import { placeStation, startPark } from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { readRide } from "../player/rides";

test("Horizon Hawk hyper coaster journey @journey", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readRide("horizon-hawk").steps.slice(0, 13);

  await startPark(page, { park: "new", seed: 2028, speed: 1 });
  await placeStation(page, { model: "hyper", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await expect(page.getByRole("combobox", { name: "Coaster model" }).locator("option:checked")).toHaveText("Horizon Hawk · Boulder & Marlin");

  // The lift is straight only, so the chain refuses a turn.
  const chain = page.getByRole("button", { name: "Chain lift", exact: true });
  await chain.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "true");
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  const turn = page.locator('[data-piece="TurnSmallStandalone"]');
  await expect(turn).toHaveClass(/piece-tile--refused/);
  await turn.click();
  await expect(page.locator("[data-construction] [data-message]")).toHaveText("No chain lift on this piece");
  await expect(page.getByRole("button", { name: "Place piece" })).toBeDisabled();
  await chain.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");

  await placeCircuit(page, circuit);
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");
  expect(pageErrors).toEqual([]);
});
