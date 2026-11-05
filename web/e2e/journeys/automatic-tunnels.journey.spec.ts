import { expect, test, type Page } from "@playwright/test";
import { clickTile, openTool, placeStation, selectUnits, setGameSpeed, startPark, testRide, waitForState } from "../player/driver";

test("a coaster tunnels through a hill and completes a test ride @journey", async ({ page }) => {
  test.setTimeout(240_000);
  await page.setViewportSize({ width: 800, height: 480 });
  await startPark(page, { park: "new", seed: 2027, speed: 1 });
  await raiseHillAcrossCoasterRoute(page);

  await placeStation(page, {
    model: "hybrid",
    height: 4,
    tile: { x: 50, y: 50, z: 2 },
    direction: { x: 52, y: 50, z: 2 },
  });

  const pieces = [
    "LevelToGentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUp", "GentleUpToLevel",
    "Straight", "OneTileStraight", "OneTileStraight", "OffsetLeft", "TurnLargeEntry", "TurnLargeExit",
    "LevelToGentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown", "GentleDown",
    "GentleDownToLevel", "Loop", "OffsetRight", "TurnLargeEntry", "TurnLargeExit", "OneTileStraight",
  ];
  expect(pieces).toContain("GentleDown");

  const choices = page.getByRole("radiogroup", { name: "Next piece" });
  const track = page.locator("[data-track] li");
  const chainLift = page.getByRole("button", { name: /Chain lift/ });
  const tunnelPreview = page.locator("[data-tunnel-preview]");
  const descendingTunnelPieces: string[] = [];
  await expect(track).toHaveCount(1);
  for (const [index, piece] of pieces.entries()) {
    if (index === 0 || index === 8) {
      await chainLift.click();
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(index === 0));
    }
    const choice = choices.locator(`[data-piece="${piece}"]`);
    await expect(choice).toBeVisible();
    await choice.click();
    const choiceRefusal = await choice.locator("[role='tooltip']").textContent();
    const choiceChecked = await choice.getAttribute("aria-checked");
    const builderMessage = await page.locator("[data-message]").textContent();
    await expect(
      page.locator("[data-construction]"),
      `${piece} was not selected. Choice refusal: ${choiceRefusal}; checked: ${choiceChecked}; builder: ${builderMessage}; ` +
        `placed pieces: ${await track.count()}`,
    ).toHaveAttribute("data-selected", piece);
    if (piece.includes("Down") && (await tunnelPreview.textContent()) === "This selection will tunnel through terrain.") {
      descendingTunnelPieces.push(piece);
    }
    const constructionMessage = await page.locator("[data-message]").textContent();
    await expect(
      page.getByRole("button", { name: "Place piece" }),
      `${piece} at route index ${index} cannot be placed. Choice: ${choiceRefusal}; builder: ${constructionMessage}; ` +
        `preview: ${await tunnelPreview.textContent()}; placed track: ${JSON.stringify(await track.allTextContents())}`,
    ).toBeEnabled();
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
  }
  expect(descendingTunnelPieces, "a descending piece should preview its tunnel").not.toEqual([]);

  const finishCoaster = page.getByRole("button", { name: "Finish coaster" });
  await expect(finishCoaster).toBeEnabled();
  await finishCoaster.click();
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-finish-preview", "true");
  await page.getByRole("button", { name: "Confirm coaster finish" }).click();
  await expect(page.getByText("Circuit complete", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();

  const tunnelSpans = await page.evaluate(() => window.gameTest.carvedSpans()
    .filter(([x, y]) => x >= 66 && x <= 74 && y >= 63 && y <= 71));
  expect(tunnelSpans.length, "track should carve the raised plateau").toBeGreaterThan(0);

  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await testRide(page);
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status"))
    .toHaveText("Train completed a lap.");

  const tunnel = tunnelSpans[0]!;
  await openTool(page, "Landscape");
  await page.getByLabel("Underground cut level").fill(String(tunnel[3] - 1));
  await page.getByRole("button", { name: "Show underground" }).click();
  await expect(page.getByRole("button", { name: "Show surface" })).toBeVisible();
  await page.evaluate(({ x, y }) => window.gameTest.setCamera({ centerX: x + 0.5, centerY: y + 0.5, zoom: 2, rotation: 0 }), {
    x: tunnel[0], y: tunnel[1],
  });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const tunnelFloor = await page.evaluate((span) => {
    const point = window.gameTest.project({ x: span[0] + 0.5, y: span[1] + 0.5, z: span[2] / 4 });
    return window.gameTest.pickSurface(point.x, point.y);
  }, tunnel);
  expect(tunnelFloor).toMatchObject({ kind: "carved-floor", x: tunnel[0], y: tunnel[1], height: tunnel[2] });
});

async function raiseHillAcrossCoasterRoute(page: Page): Promise<void> {
  await openTool(page, "Landscape");
  await selectUnits(page, "metric");
  const landscaping = page.getByRole("region", { name: "Landscaping" });
  await landscaping.getByRole("button", { name: "Flatten", exact: true }).click();
  await page.getByLabel("Brush size").selectOption("9");
  await page.getByLabel("Flatten height").fill("20");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 70.5, centerY: 67.5, zoom: 1.5, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  const status = landscaping.locator(".landscaping__status");
  for (const tile of [{ x: 70, y: 67 }]) {
    await landscaping.getByRole("button", { name: "Apply tool" }).click();
    await clickTile(page, { ...tile, z: 2 });
    await expect(status).toHaveText("Terrain updated");
    await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  }
  await landscaping.getByRole("button", { name: "Cancel tool" }).click();
}

