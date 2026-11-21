import { expect, test } from "@playwright/test";
import { flattenTerrainTiles, placeStation, startPark } from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { readRide } from "../player/rides";

test("Horizon Hawk hyper coaster journey @journey", async ({ page }) => {
  test.setTimeout(360_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setViewportSize({ width: 800, height: 480 });
  const ride = readRide("horizon-hawk");
  const circuit = ride.steps.slice(0, 13);
  const circuitPieces = ride.steps.flatMap((step) => "place" in step ? [step.place] : []);
  expect(circuitPieces).toContain("WideLevelToSixtyDown");
  const shapedHills = ride.steps.filter((step) => "intent" in step
    && "hill" in step.intent
    && step.intent.hill.feel === "StrongFloater");
  expect(shapedHills.length).toBeGreaterThanOrEqual(4);
  expect(shapedHills[0]).toMatchObject({
    intent: { hill: { feel: "StrongFloater", strength_g: -0.85, size: { Height: 24 } } },
  });
  expect(shapedHills.every((step) => "intent" in step
    && "hill" in step.intent
    && step.intent.hill.strength_g <= -0.8)).toBe(true);
  const shapedTurns = ride.steps.filter((step) => "intent" in step && "turn" in step.intent);
  expect(shapedTurns.length).toBeGreaterThanOrEqual(7);
  expect(shapedTurns.some((step) => "intent" in step && "turn" in step.intent && step.intent.turn.hand === "Left")).toBe(true);
  expect(shapedTurns.some((step) => "intent" in step && "turn" in step.intent && step.intent.turn.hand === "Right")).toBe(true);
  expect(ride.expect?.airtimeMomentsAtLeast).toBeGreaterThanOrEqual(6);

  await startPark(page, { park: "new", seed: 2028, speed: 1 });
  const defaultParkSurfaceHeight = 8;
  const minimumStationHeight = 4;
  const adjustedStationSurfaceHeight = ride.station.z - minimumStationHeight;
  expect(adjustedStationSurfaceHeight).toBe(defaultParkSurfaceHeight - 2);
  await flattenTerrainTiles(page, [{
    x: ride.station.x,
    y: ride.station.y,
    height: adjustedStationSurfaceHeight,
  }]);
  await placeStation(page, {
    model: "hyper",
    height: minimumStationHeight,
    tile: { x: ride.station.x, y: ride.station.y, z: 2 },
    direction: { x: ride.station.x, y: ride.station.y + 2, z: 2 },
  });
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
