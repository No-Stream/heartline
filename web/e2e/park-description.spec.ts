import { expect, test } from "@playwright/test";
import { openTool } from "./game-page";
import { readRide } from "./player/rides";

const horizonHawkGeometryDigests = [
  "0f412105af8180cd",
  "156aea70a14dd282",
  "64205777d10a8403",
  "9ab5644ea170f306",
  "f6c21b4e00293d74",
  "8a064b5558ba6430",
  "03d052c69bcbcd76",
  "1b61c8c42060df64",
  "fd30c396b91df5c1",
  "d5072151e8c659ac",
  "f8505135b93845a9",
  "aed4591031f24b1f",
  "09bb59f9b0d6c9f3",
  "6a3eb490de024f8c",
  "3a47c2875768274b",
  "f3c33f0f5f22025d",
  "a569e62c290fbe7f",
  "082463ec8b50000f",
];

test("a bundled park description loads and guests ride its coaster @slow", async ({ page }) => {
  test.setTimeout(240_000);
  const ride = readRide("horizon-hawk");
  const shapedDigests = ride.steps.flatMap((step) => {
    if (!("intent" in step)) return [];
    return [BigInt(step.spec.geometry_digest).toString(16).padStart(16, "0")];
  });
  expect(shapedDigests).toEqual(horizonHawkGeometryDigests);

  await page.goto("/?park=horizon-hawk&test=1&agents=12");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);

  const initialState = await page.evaluate(() => ({
    facilities: window.gameTest.facilities().map(({ kind }) => kind),
    paths: window.gameTest.paths(),
  }));
  expect(initialState.facilities).toContain("entrance");
  expect(initialState.paths.length).toBeGreaterThan(0);
  expect(initialState.paths.some(({ kind }) => kind === 1), "the loaded coaster queue should be present").toBe(true);

  let riderCount = 0;
  for (let simulatedTicks = 0; simulatedTicks < 30_000 && riderCount === 0; simulatedTicks += 1_000) {
    await page.evaluate(() => window.gameTest.advanceTicks(1_000));
    riderCount = await page.evaluate(() => window.gameTest.riders().length);
  }
  expect(riderCount, "a guest should board the coaster loaded from the park description").toBeGreaterThan(0);

  await openTool(page, "Watch");
  await page.getByRole("button", { name: "Export session" }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toContainText("Session export is unavailable after loading a park");
});
