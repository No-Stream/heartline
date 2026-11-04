import { expect, test } from "@playwright/test";
import { readCircuit } from "../player/circuits";
import { closeCircuit, openTool, placePiece, placeStation, setGameSpeed, startPark, testRide, waitForState } from "../player/driver";

test("Briar Blitz player journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setViewportSize({ width: 800, height: 480 });
  await startPark(page, { park: "new", seed: 2027, speed: 1 });
  await placeStation(page, {
    model: "wooden_twister",
    height: 4,
    tile: { x: 241, y: 249, z: 2 },
    direction: { x: 241, y: 251, z: 2 },
  });

  const pieces = readCircuit("Briar Blitz");
  const trackPieces = pieces.filter((piece) => piece !== "+chain" && piece !== "-chain");
  const chainOn = pieces.indexOf("+chain");
  const chainOff = pieces.indexOf("-chain");
  const curvedLiftTurn = pieces.indexOf("TurnSmallStandalone");
  const descendingBankedTurnIndexes = pieces.flatMap((piece, index) =>
    piece === "BankedTurnStandalone" ? [index] : [],
  );
  expect(chainOn).toBeGreaterThanOrEqual(0);
  expect(chainOn).toBeLessThan(curvedLiftTurn);
  expect(chainOff).toBeGreaterThan(curvedLiftTurn);
  expect(descendingBankedTurnIndexes.length).toBeGreaterThan(0);
  expect(descendingBankedTurnIndexes.every((index) => index > chainOff)).toBe(true);
  expect(trackPieces.filter((piece) => piece.startsWith("TightTwister") || piece.startsWith("Twister")).length).toBeGreaterThanOrEqual(2);
  const previewName = page.locator("[data-next] [data-preview-name]");
  let chainEnabled = false;
  let trackCount = 1;
  const placedPieces: string[] = [];
  for (const entry of pieces) {
    if (entry === "+chain" || entry === "-chain") {
      chainEnabled = entry === "+chain";
      await page.keyboard.press("h");
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(chainEnabled));
      continue;
    }

    await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(chainEnabled));
    const choice = page.locator(`[data-piece="${entry}"]`);
    await expect(choice, `Briar Blitz does not offer ${entry}`).toBeVisible();
    await choice.click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", entry);
    if (entry === "TurnSmallStandalone" && chainEnabled) {
      await expect(previewName).toHaveText("Climbing standalone small left turn");
    }
    if (entry === "BankedTurnStandalone" && !chainEnabled) {
      await expect(previewName).toHaveText("Descending standalone small left-banked left turn");
    }
    if (entry === "TightTwisterLeft") await expect(previewName).toHaveText("Tight left turn");
    if (entry === "TightTwisterRight") await expect(previewName).toHaveText("Tight right turn");
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await placePiece(page);
    placedPieces.push(entry);
    trackCount += 1;
    await expect(page.locator("[data-track] li")).toHaveCount(trackCount);
  }
  expect(chainEnabled).toBe(false);
  expect(trackCount).toBe(trackPieces.length + 1);
  expect(placedPieces).toContain("BankedTurnStandalone");
  expect(placedPieces.filter((piece) => piece.startsWith("TightTwister") || piece.startsWith("Twister")).length).toBeGreaterThanOrEqual(2);

  await page.setViewportSize({ width: 1280, height: 720 });
  await closeCircuit(page);
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  const crestCount = await page.evaluate(() => window.gameTest.crestBursts().length);
  await testRide(page);
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  const notes = await page.evaluate(() => window.gameTest.testingReport()!.notes);
  expect(notes.join(" ")).not.toMatch(/could not finish|rolled back/i);
  expect(await page.evaluate(() => window.gameTest.crestBursts().length)).toBeGreaterThan(crestCount);
  expect(pageErrors).toEqual([]);
});
