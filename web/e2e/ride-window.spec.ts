import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { parseReplaySession } from "../src/session-replay";
import { clickCanvas, openTool, selectUnits } from "./game-page";

interface CoasterPick {
  readonly id: string;
  readonly x: number;
  readonly y: number;
}

async function findDemoCoaster(page: Page): Promise<CoasterPick> {
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 228, centerY: 252, zoom: 0.5, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const target = await page.evaluate((): CoasterPick | null => {
    const canvas = document.querySelector("canvas");
    if (!canvas) return null;
    for (let y = 0; y < canvas.height; y += 4) {
      for (let x = 0; x < canvas.width; x += 4) {
        const picked = window.gameTest.pick(x, y);
        if (picked?.kind === "track" && "id" in picked && /^track-\d+$/.test(picked.id)) return { id: picked.id, x, y };
      }
    }
    return null;
  });
  if (!target) throw new Error("No demo coaster track could be picked from the rendered world");
  return target;
}

test("clicking the demo coaster opens a ride window with its scores, statistics and g-force graph @fast", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await selectUnits(page, "metric");
  await clickCanvas(page, await findDemoCoaster(page));

  const rideWindow = page.getByTestId("ride-window");
  await expect(rideWindow).toBeVisible();
  await expect(rideWindow.getByTestId("ride-name")).toHaveText("Timber Tumbler");
  await expect(rideWindow.getByTestId("ride-untested")).toBeVisible();
  await expect(rideWindow.getByTestId("ride-report")).toBeHidden();

  await rideWindow.getByTestId("ride-test").click();
  await expect(rideWindow.getByTestId("ride-report")).toBeVisible();
  await expect(rideWindow.getByTestId("ride-untested")).toBeHidden();

  const report = await page.evaluate(() => window.gameTest.rideInfo()!.report!);
  for (const score of ["excitement", "intensity", "nausea"] as const) {
    await expect(rideWindow.locator(`[data-score="${score}"] .testing-score__value`)).toHaveText(report.rating[score].toFixed(1).replace("-", "−"));
  }
  const maxSpeedRow = rideWindow.locator("[data-statistics-speed-layout] > div", { hasText: "Max speed" });
  await expect(maxSpeedRow.locator("dd")).toHaveText(/ km\/h$/);
  expect(Number.parseFloat((await maxSpeedRow.locator("dd").innerText()).replaceAll(",", ""))).toBeCloseTo(report.statistics.maxSpeed * 3.6, 1);
  await expect(rideWindow.locator("[data-score-findings] .testing-finding").first()).toBeVisible();

  const graph = rideWindow.getByTestId("ride-graph");
  await expect(graph).toBeVisible();
  await expect(graph).toHaveAttribute("data-sample-count", String(report.samples.length));
  for (const series of ["verticalG", "lateralG", "longitudinalG", "speed"]) {
    await expect(graph.locator(`path[data-series="${series}"]`)).toHaveAttribute("d", /^M[\d.]+ [\d.]+ L/);
  }
  await expect(graph.locator(".ride-graph__band")).toHaveCount(3);
  const readout = rideWindow.getByTestId("ride-graph-readout");
  await expect(readout).toContainText("Point at the graph");

  const bounds = (await graph.boundingBox())!;
  await page.mouse.move(bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5);
  await expect(readout).toContainText(/^\d+\.\d s · Speed \d+ km\/h · Vertical −?\d+\.\d g · Lateral −?\d+\.\d g · Forward −?\d+\.\d g · [\d,]+ m along the track$/);
  await expect(graph.getByTestId("ride-graph-cursor")).toBeVisible();
  const hoveredIndex = Number(await graph.getAttribute("data-cursor-index"));
  expect(hoveredIndex).toBeGreaterThan(0);
  expect(hoveredIndex).toBeLessThan(report.samples.length - 1);
  const hovered = report.samples[hoveredIndex];
  await expect(readout).toContainText(`Vertical ${hovered.verticalG.toFixed(1).replace("-", "−")} g`);

  const projected = await page.evaluate((sample) => window.gameTest.project(sample), hovered);
  await expect.poll(() => page.evaluate(({ x, y }) => window.gameTest.pickRegion(Math.round(x) - 3, Math.round(y) - 3, 7, 7).some((pick) => pick?.kind === "testing"), projected)).toBe(true);

  await page.mouse.move(bounds.x - 20, bounds.y - 20);
  await expect(readout).toContainText("Point at the graph");
  await expect.poll(() => page.evaluate(({ x, y }) => window.gameTest.pickRegion(Math.round(x) - 3, Math.round(y) - 3, 7, 7).some((pick) => pick?.kind === "testing"), projected)).toBe(false);

  await page.keyboard.press("Escape");
  await expect(rideWindow).toBeHidden();
});

test("the ride window shows distances and speeds in the chosen units @fast", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await selectUnits(page, "imperial");
  await clickCanvas(page, await findDemoCoaster(page));
  const rideWindow = page.getByTestId("ride-window");
  await rideWindow.getByTestId("ride-test").click();
  await expect(rideWindow.locator("[data-statistics-speed-layout] > div", { hasText: "Track length" }).locator("dd")).toHaveText(/ ft$/);
  await rideWindow.getByTestId("ride-graph").hover();
  await expect(rideWindow.getByTestId("ride-graph-readout")).toContainText(/ mph · .* ft along the track$/);
  await selectUnits(page, "metric");
  await expect(rideWindow.locator("[data-statistics-speed-layout] > div", { hasText: "Track length" }).locator("dd")).toHaveText(/ m$/);
});

test("the builder's Ride info button and a click on the built coaster open the same window, which keeps its report", async ({ page }) => {
  test.setTimeout(120_000);
  const session = parseReplaySession(JSON.parse(readFileSync(new URL("./sessions/timber-tumbler-built.json", import.meta.url), "utf8")) as unknown);
  const params = new URLSearchParams({ test: "1", manualBlockAnalysis: "1", park: session.park, seed: session.seed, agents: String(session.guestCount) });
  await page.goto(`/?${params}`);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate((recordedSession) => window.gameTest.replaySession(recordedSession), session);

  await openTool(page, "Coaster");
  await page.getByRole("button", { name: "Ride info", exact: true }).click();
  const rideWindow = page.getByTestId("ride-window");
  await expect(rideWindow).toHaveAttribute("data-ride", "player");
  await expect(rideWindow.getByTestId("ride-name")).toHaveText("Timber Tumbler");
  // The recorded session ends with a finished test run, so the window shows its report at once.
  await expect(rideWindow.getByTestId("ride-report")).toBeVisible();
  await expect(rideWindow.getByTestId("ride-untested")).toBeHidden();
  const report = await page.evaluate(() => window.gameTest.rideInfo()!.report!);
  await expect(rideWindow.locator('[data-score="excitement"] .testing-score__value')).toHaveText(report.rating.excitement.toFixed(1));
  await rideWindow.getByTestId("ride-window-close").click();
  await expect(rideWindow).toBeHidden();

  await openTool(page, "Watch");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 236, centerY: 246, zoom: 1.1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const target = await page.evaluate((): CoasterPick | null => {
    const canvas = document.querySelector("canvas")!;
    for (let y = 4; y < canvas.height; y += 4) {
      for (let x = 4; x < canvas.width; x += 4) {
        const picked = window.gameTest.pick(x, y);
        if (picked?.kind === "track" && "id" in picked && picked.id.startsWith("construction-track-")) return { id: picked.id, x, y };
      }
    }
    return null;
  });
  if (!target) throw new Error("The built coaster's track could not be picked");
  await clickCanvas(page, target);
  await expect(rideWindow).toHaveAttribute("data-ride", "player");
  await expect(rideWindow.getByTestId("ride-report")).toBeVisible();
  await expect(rideWindow.locator('[data-score="excitement"] .testing-score__value')).toHaveText(report.rating.excitement.toFixed(1));
});
