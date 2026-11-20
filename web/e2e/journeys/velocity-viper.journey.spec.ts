import { test } from "@playwright/test";
import { placeCircuit } from "../player/place-circuit";
import { placeStation, startPark } from "../player/driver";
import { readRide } from "../player/rides";

test("Velocity Viper keyboard construction journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readRide("velocity-viper").steps.slice(0, 8);
  await startPark(page, { park: "new", seed: 2026, speed: 1, livePauseRender: true });
  await placeStation(page, { model: "velocity_viper", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await placeCircuit(page, circuit, true);
});
