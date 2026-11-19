import { expect, test } from "@playwright/test";
import { placeStation, startPark } from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { readRide } from "../player/rides";

test("Daisy Dash family coaster journey @journey", async ({ page }) => {
  test.setTimeout(240_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readRide("daisy-dash").steps.slice(0, 13);

  await startPark(page, { park: "new", seed: 2027, speed: 1 });
  await placeStation(page, { model: "family_coaster", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await expect(page.getByRole("combobox", { name: "Coaster model" }).locator("option:checked")).toHaveText("Daisy Dash · Windmill Ride Works");
  await placeCircuit(page, circuit);
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");
  expect(pageErrors).toEqual([]);
});
