import { expect, test } from "@playwright/test";
import { readCircuit } from "../player/circuits";
import { fileURLToPath } from "node:url";
import { closeCircuit, layPath, openTool, placeFacility, placePiece as placeSelectedPiece, placeStation, setGameSpeed, startPark, testRide, waitForState } from "../player/driver";

test("Timber Tumbler player journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  let phaseStarted = Date.now();
  const phase = (name: string): void => {
    console.log(`Journey phase ${name}: ${Date.now() - phaseStarted} ms`);
    phaseStarted = Date.now();
  };
  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readCircuit("Timber Tumbler");
  let circuitIndex = 0;
  let chain = false;
  const placePiece = async (): Promise<void> => {
    while (circuit[circuitIndex] === "+chain" || circuit[circuitIndex] === "-chain") {
      chain = circuit[circuitIndex++] === "+chain";
    }
    const piece = circuit[circuitIndex++];
    if (!piece) throw new Error("Timber Tumbler circuit is missing a piece");
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(chain));
    await expect(page.locator('[data-piece][aria-checked="true"]')).toHaveAttribute("data-piece", piece);
    await placeSelectedPiece(page);
  };
  const previewName = page.locator("[data-next] [data-preview-name]");
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await layPath(page, { x: 218, y: 251, z: 2 }, { x: 218, y: 256, z: 2 });
  await placeFacility(page, "Food stall", { x: 216, y: 252, z: 2 });
  await placeFacility(page, "Drink stall", { x: 216, y: 254, z: 2 });
  await placeFacility(page, "Restroom", { x: 216, y: 256, z: 2 });
  phase("park setup");
  await placeStation(page, { model: "hybrid", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await page.keyboard.press("h");
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "true");
  await page.getByRole("radio", { name: "Begin gentle climb", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  await page.getByRole("radio", { name: "Gentle climb", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Turn left", exact: true }), "no climbing turn offered while the chain lift climbs").toBeEnabled();
  await page.getByRole("button", { name: "Turn left", exact: true }).click();
  await expect(previewName).toHaveText("Climbing small left turn");
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(4);

  await page.getByRole("button", { name: "Turn right", exact: true }).click();
  await expect(previewName).toHaveText("Gentle climb");
  await page.getByRole("button", { name: "Slope up", exact: true }).click();
  await expect(previewName).toHaveText("Begin steep climb");
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(5);

  await page.getByRole("radio", { name: "Steep climb", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Slope up", exact: true }),
    "the 60-degree climb transition is not offered from the 45-degree chain lift",
  ).toBeEnabled();
  await page.getByRole("button", { name: "Slope up", exact: true }).click();
  await expect(previewName).toHaveText("Begin 60-degree climb");
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(6);
  await expect(previewName).toHaveText("60-degree climb");
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(7);

  await page.getByRole("radio", { name: "End 60-degree climb", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(8);
  await page.getByRole("radio", { name: "End steep climb", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(9);
  await page.getByRole("radio", { name: "End gentle climb", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(10);
  await page.keyboard.press("h");
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", "false");

  await page.getByRole("radio", { name: "Begin gentle descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(11);
  await page.getByRole("radio", { name: "Begin steep descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(12);
  await page.getByRole("radio", { name: "Steep descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(13);
  await page.getByRole("radio", { name: "Begin 60-degree descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(14);
  await page.getByRole("radio", { name: "60-degree descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(15);
  await page.getByRole("radio", { name: "End 60-degree descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(16);
  await page.getByRole("radio", { name: "End steep descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(17);
  await page.getByRole("radio", { name: "End gentle descent", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(18);

  await page.getByRole("radio", { name: "Bank left", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(19);
  await page.getByRole("radio", { name: "Small left-banked turn", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(20);
  await page.getByRole("radio", { name: "Small left-banked turn", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(21);
  await page.getByRole("radio", { name: "Level from left bank", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(22);
  await page.getByRole("radio", { name: "Small airtime hill", exact: true }).click();
  await placePiece();
  await expect(page.locator("[data-track] li")).toHaveCount(23);

  const returnPieces = [
    "Begin gentle climb", "Climbing small right turn", "End gentle climb",
    "Begin gentle descent", "Gentle descent", "Gentle descent", "End gentle descent", "Small left turn",
    ...Array<string>(7).fill("Straight"), "Large left turn", ...Array<string>(3).fill("Straight"),
    "Straight", "Straight", "Small left turn", "One-tile straight",
    "Large left turn", "Large left turn", "Small left turn",
  ];
  for (const [index, name] of returnPieces.entries()) {
    if (index === 0 || name !== returnPieces[index - 1]) await page.getByRole("radio", { name, exact: true }).click();
    await placePiece();
    await expect(page.locator("[data-track] li")).toHaveCount(index + 24);
  }
  expect(circuitIndex).toBe(circuit.length);
  phase("construction");
  await page.setViewportSize({ width: 1280, height: 720 });
  await closeCircuit(page);
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await testRide(page);
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  const notes = await page.evaluate(() => window.gameTest.testingReport()!.notes);
  expect(notes.join(" ")).not.toMatch(/could not finish|rolled back/i);

  phase("ride test");
  if (process.env.RECORD_JOURNEY_SESSIONS === "1") {
    await openTool(page, "Watch");
    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Export session", exact: true }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toMatch(/^session-.*\.json$/);
    await download.saveAs(fileURLToPath(new URL("../sessions/timber-tumbler-built.json", import.meta.url)));
  }
});
