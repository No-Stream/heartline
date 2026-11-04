import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import { canvasToPage, clickCanvas, moveOverCanvas, openTool, selectUnits } from "./game-page";

interface ExportedSession {
  format: string;
  version: number;
  commands: { tick: number; command: { type: string } }[];
  finalTick: number;
}

test("exports the UI-built command count and final simulation tick", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("pageerror", (error) => browserErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") browserErrors.push(message.text());
  });
  await page.goto("/?test=1&manualBlockAnalysis=1&park=new&seed=2026");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);

  await openTool(page, "Coaster");
  await selectUnits(page, "metric");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.getByLabel("Station height above ground (metres)").fill("72");
  await page.getByLabel("Station height above ground (metres)").press("Tab");
  await page.getByRole("button", { name: "Place station" }).click();
  const canvas = page.locator("canvas");
  const station = await page.evaluate(() => window.gameTest.project({ x: 50.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, station);
  await expect(canvas).toHaveAttribute("data-station-preview", /^50,50,\d+,\d+$/);
  await clickCanvas(page, station);
  const direction = await page.evaluate(() => window.gameTest.project({ x: 52.5, y: 50.5, z: 2 }));
  await moveOverCanvas(page, direction);
  await expect(canvas).toHaveAttribute("data-station-preview", /^50,50,\d+,0$/);
  await clickCanvas(page, direction);
  await expect(canvas).toHaveAttribute("data-station-preview", "");
  const track = page.locator("[data-track] li");
  await expect(track, `Station did not start construction. Browser errors: ${browserErrors.join(" | ")}`).toHaveCount(1);

  const expectedCommandTypes = [
    "construction-start",
    "construction-select",
    "construction-append",
    "construction-select",
    "construction-append",
  ];
  const pieces = ["TurnSmallStandalone", "Straight"] as const;
  for (const [index, piece] of pieces.entries()) {
    await page.locator(`[data-piece="${piece}"]`).click();
    await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", piece);
    await page.getByRole("button", { name: "Place piece" }).click();
    await expect(track).toHaveCount(index + 2);
  }

  const tickBeforeAdvance = await page.evaluate(() => window.gameTest.tickCount());
  await page.evaluate(() => window.gameTest.advanceTicks(37));
  const observedFinalTick = await page.evaluate(() => window.gameTest.tickCount());
  expect(observedFinalTick).toBe(tickBeforeAdvance + 37);
  const tickReadout = page.getByTestId("tick-count");
  await expect(tickReadout).toHaveText(String(observedFinalTick));

  const exportButton = page.getByRole("button", { name: "Export session", exact: true });
  await openTool(page, "Watch");
  await expect(exportButton).toBeVisible();
  const downloadPromise = page.waitForEvent("download");
  await exportButton.click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toMatch(/^session-.*\.json$/);
  const downloadPath = await download.path();
  if (!downloadPath) throw new Error("Session export download has no file path");
  const session = JSON.parse(await readFile(downloadPath, "utf8")) as ExportedSession;

  expect(session.format).toBe("theme-park-session");
  expect(session.version).toBe(1);
  expect(session.commands).toHaveLength(expectedCommandTypes.length);
  expect(session.commands.map(({ command }) => command.type)).toEqual(expectedCommandTypes);
  expect(session.commands.every(({ tick }) => tick === tickBeforeAdvance)).toBe(true);
  expect(session.finalTick).toBe(observedFinalTick);
});
