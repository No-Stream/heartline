import { expect, test } from "@playwright/test";

test("builds a closed coaster and runs a train around it", async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 30, centerY: 30, zoom: 1, rotation: 0 }));
  await page.getByLabel("Station X (tiles)").fill("50");
  await page.getByLabel("Station Y (tiles)").fill("50");
  await page.getByLabel("Station rail height (metres)").fill("80");
  await page.getByRole("button", { name: "Start coaster" }).click();
  const choices = page.getByLabel("Next piece");
  const track = page.locator("[data-track]");
  await expect(track.locator("li")).toHaveCount(1);
  const pieces = [
    "LiftLevelToGentleUp", "Lift", "Lift", "Lift", "Lift", "Lift", "LiftGentleUpToLevel",
    "LiftFlat", "Straight", "Straight", "TurnLargeEntry", "TurnLargeExit",
    "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown",
    "GentleDownToLevel", "Loop", "TurnLargeEntry", "TurnLargeExit",
  ];
  for (const [index, piece] of pieces.entries()) {
    await expect(choices.locator(`option[value="${piece}"]`)).toHaveCount(1);
    await choices.selectOption(piece);
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    if (index === 0) {
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      await page.addStyleTag({ content: "[data-testid='tick-count'] { visibility: hidden; }" });
      await expect(page.locator("main")).toHaveScreenshot("construction-preview.png", { animations: "disabled" });
    }
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track.locator("li")).toHaveCount(index + 2);
  }
  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await page.getByRole("button", { name: "Test coaster" }).click();
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status")).toHaveText("Train completed a lap.", { timeout: 150_000 });
  await page.getByRole("button", { name: "Delete LiftLevelToGentleUp, piece 2" }).click();
  await expect(track.locator("li")).toHaveCount(pieces.length, { timeout: 30_000 });
  await expect(track.locator("li").last()).toContainText("TurnLargeExit");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeDisabled();
});
