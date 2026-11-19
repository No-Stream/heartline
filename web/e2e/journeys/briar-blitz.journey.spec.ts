import { expect, test } from "@playwright/test";
import { placePiece, placeStation, startPark } from "../player/driver";
import { readRide } from "../player/rides";

test("Briar Blitz player journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setViewportSize({ width: 800, height: 480 });
  await startPark(page, { park: "new", seed: 2027, speed: 1, livePauseRender: true });
  await page.getByRole("button", { name: "Pause" }).click();
  await placeStation(page, {
    model: "wooden_twister",
    height: 4,
    tile: { x: 241, y: 249, z: 2 },
    direction: { x: 241, y: 251, z: 2 },
  });

  const pieces = readRide("briar-blitz").steps.slice(0, 14).map((step) => {
    if ("place" in step) return step.place;
    if ("chain" in step) return step.chain ? "+chain" : "-chain";
    throw new Error("Briar Blitz journey fixture contains a non-placement step");
  });
  const trackPieces = pieces.filter((piece) => piece !== "+chain" && piece !== "-chain");
  const previewName = page.locator("[data-next] [data-preview-name]");
  let chainEnabled = false;
  let trackCount = 1;
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
      await expect(previewName).toHaveText("Climbing small left turn");
    }
    if (entry === "BankedTurnStandalone" && !chainEnabled) {
      await expect(previewName).toHaveText("Descending small left-banked turn");
    }
    if (entry === "TightTwisterLeft") await expect(previewName).toHaveText("Tight left turn");
    if (entry === "TightTwisterRight") await expect(previewName).toHaveText("Tight right turn");
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await placePiece(page);
    trackCount += 1;
    await expect(page.locator("[data-track] li")).toHaveCount(trackCount);
  }
  expect(chainEnabled).toBe(false);
  expect(trackCount).toBe(trackPieces.length + 1);
  expect(pageErrors).toEqual([]);
});
