import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { readCircuit } from "../player/circuits";
import { fileURLToPath } from "node:url";
import { clickCanvas, closeCircuit, legacyRideRating, layPath, openTool, placeFacility, placePiece as placeSelectedPiece, placeStation, selectUnits, setGameSpeed, startPark, testRide, waitForState } from "../player/driver";
import { expectRideReportExplained } from "../player/ride-report";
import { parseReplaySession } from "../../src/session-replay";

test("Timber Tumbler player journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  let phaseStarted = Date.now();
  const phase = (name: string): void => {
    console.log(`Journey phase ${name}: ${Date.now() - phaseStarted} ms`);
    phaseStarted = Date.now();
  };
  await page.setViewportSize({ width: 800, height: 480 });
  const circuit = readCircuit("Timber Tumbler");
  const hillTargets: Record<string, { feel: string; twist: string; height: number; span: number }> = {
    EjectorHill16x96: { feel: "Ejector", twist: "Upright", height: 16, span: 96 },
    EjectorHill12x80OuterLeft: { feel: "Ejector", twist: "Outer bank left", height: 12, span: 80 },
    ZeroGRollFastLeft: { feel: "Zero-g roll", twist: "Roll left", height: 8, span: 86 },
    ZeroGRollFastRight: { feel: "Zero-g roll", twist: "Roll right", height: 8, span: 86 },
    StallLeft: { feel: "Stall", twist: "Roll left", height: 13, span: 104 },
    FloaterHill12x96: { feel: "Floater", twist: "Upright", height: 12, span: 96 },
    FloaterHill12x80: { feel: "Floater", twist: "Upright", height: 12, span: 80 },
  };
  const hillOrders: Record<string, string[]> = {
    feel: ["Ejector", "Floater", "Speed hill", "Zero-g roll", "Stall"],
    twist: ["Outer bank left", "Upright", "Outer bank right", "Roll left", "Roll right"],
  };
  const selectHill = async (piece: string): Promise<void> => {
    const target = hillTargets[piece];
    const hill = page.locator("[data-hill]");
    if (await hill.getAttribute("open") === null) await hill.locator("summary").click();
    for (const attribute of ["feel", "twist", "height", "span"] as const) {
      const value = hill.locator(`[data-hill-value="${attribute}"]`);
      await expect(value).not.toBeEmpty();
      const desired = target[attribute];
      for (let step = 0; step < 8; step += 1) {
        const current = (await value.textContent())?.trim();
        if (!current) throw new Error(`Missing ${attribute} for ${piece}`);
        if (current === String(desired) || current === `${desired} m`) break;
        const order = hillOrders[attribute];
        const currentRank = order ? order.indexOf(current) : Number.parseFloat(current);
        const targetRank = order ? order.indexOf(String(desired)) : Number(desired);
        if (currentRank < 0 || targetRank < 0 || !Number.isFinite(currentRank)) throw new Error(`Unknown ${attribute}: ${current}`);
        const direction = currentRank < targetRank ? 1 : -1;
        await hill.locator(`[data-hill-stepper="${attribute}-${direction}"]`).click();
        await expect(value).not.toHaveText(current);
      }
      await expect(value).toHaveText(typeof desired === "number" ? `${desired} m` : desired);
    }
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
  };
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await layPath(page, { x: 218, y: 251, z: 2 }, { x: 218, y: 256, z: 2 });
  await placeFacility(page, "Food stall", { x: 216, y: 252, z: 2 });
  await placeFacility(page, "Drink stall", { x: 216, y: 254, z: 2 });
  await placeFacility(page, "Restroom", { x: 216, y: 256, z: 2 });
  phase("park setup");
  await placeStation(page, { model: "hybrid", height: 4, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });

  let chainEnabled = false;
  let trackCount = 1;
  for (const entry of circuit) {
    if (entry === "+chain" || entry === "-chain") {
      chainEnabled = entry === "+chain";
      await page.keyboard.press("h");
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(chainEnabled));
      continue;
    }
    if (entry in hillTargets) {
      await selectHill(entry);
    } else {
      const choice = page.locator(`[data-piece="${entry}"]`);
      await expect(choice, `Timber Tumbler does not offer ${entry}`).toBeVisible();
      await choice.click();
      await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
      const variant = page.locator(`[data-speed-match-variant="${entry}"]`);
      if (await variant.isVisible()) await variant.click();
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", entry);
    }
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(chainEnabled));
    await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
    await placeSelectedPiece(page);
    await expect(page.locator("[data-track] li")).toHaveCount(++trackCount);
  }
  expect(chainEnabled).toBe(false);
  phase("construction");
  await page.setViewportSize({ width: 1280, height: 720 });
  await closeCircuit(page);
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
  await setGameSpeed(page, 16);
  await openTool(page, "Coaster");
  await testRide(page);
  await waitForState(page, async () => (await page.getByRole("region", { name: "Coaster construction" }).getByRole("status").textContent()) === "Train completed a lap.", { ticks: 30_000 });
  await waitForState(page, async () => (await page.evaluate(() => window.gameTest.testingReport()?.rating)) != null, { ticks: 30_000 });
  await expect(page.locator('[data-score-finding="airtime"] .testing-effect[data-score="excitement"]')).toHaveText(/^Excitement \+/);
  const { rating } = await expectRideReportExplained(page);
  console.log(`Timber Tumbler ratings before/after: ${JSON.stringify({ before: legacyRideRating(rating), after: rating })}`);
  const notes = await page.evaluate(() => window.gameTest.testingReport()!.notes);
  expect(notes.join(" ")).not.toMatch(/could not finish|rolled back/i);

  const maximumHeight = page.locator("[data-report-maximum-height]");
  const metricMaximumHeight = Number((await maximumHeight.textContent())?.match(/\d+(?:\.\d+)?/)?.[0]);
  await selectUnits(page, "imperial");
  const imperialMaximumHeight = Number((await maximumHeight.textContent())?.match(/\d+(?:\.\d+)?/)?.[0]);
  expect(metricMaximumHeight).toBeCloseTo(imperialMaximumHeight * 0.3048, 0);
  await selectUnits(page, "metric");

  await page.getByLabel("Track overlay").selectOption("speed");
  const overlaySize = await page.locator("canvas").evaluate((canvas: HTMLCanvasElement) => ({ width: canvas.width, height: canvas.height }));
  const overlayPick = await page.evaluate((size) => {
    const report = window.gameTest.testingReport();
    if (!report) throw new Error("Missing coaster test report");
    for (const sample of report.samples) {
      const point = window.gameTest.project(sample);
      if (point.x < 0 || point.x >= size.width || point.y < 0 || point.y >= size.height) continue;
      if (window.gameTest.pick(point.x, point.y)?.kind === "testing") return point;
    }
    throw new Error("No speed overlay segment was pickable");
  }, overlaySize);
  await clickCanvas(page, overlayPick);
  await expect(page.locator("[data-inspection]")).toContainText("Vertical:");

  phase("ride test");
  if (process.env.RECORD_JOURNEY_SESSIONS === "1") {
    await openTool(page, "Watch");
    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Export session", exact: true }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toMatch(/^session-.*\.json$/);
    const recordingPath = await download.path();
    if (!recordingPath) throw new Error("The exported journey session is unavailable");
    const session = parseReplaySession(JSON.parse(await readFile(recordingPath, "utf8")) as unknown);
    const sessionPath = fileURLToPath(new URL("../sessions/timber-tumbler-built.json", import.meta.url));
    await writeFile(sessionPath, `${JSON.stringify({ ...session, expect: { constructionClosed: true } }, null, 2)}\n`);
    await openTool(page, "Coaster");
  }

  const estimateBeforeDeletion = await page.evaluate(() => window.gameTest.constructionForcePreview());
  await page.getByRole("button", { name: "Delete Begin steep climb, piece 3" }).click();
  await expect(page.locator("[data-track] li")).toHaveCount(trackCount - 1);
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  await expect(page.getByRole("button", { name: "Test coaster" })).toBeDisabled();
  expect(await page.evaluate(() => window.gameTest.testingReport())).toBeNull();
  const estimateAfterDeletion = await page.evaluate(() => window.gameTest.constructionForcePreview());
  expect(estimateAfterDeletion?.samples.length).toBeGreaterThan(0);
  expect(estimateAfterDeletion).not.toEqual(estimateBeforeDeletion);
  await expect(page.getByLabel("Track overlay")).toBeEnabled();
});
