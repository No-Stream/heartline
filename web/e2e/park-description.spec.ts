import { expect, test } from "@playwright/test";
import { openTool } from "./game-page";
import { readRide } from "./player/rides";

const horizonHawkGeometryDigests = [
  "0f412105af8180cd",
  "b5fdb605bdac02c8",
  "3b36b9805f13f147",
  "d3ab2d19dc77d285",
  "067e0677470869e7",
  "23f0dc11f2e0cd49",
  "431463f9b9d666e1",
  "05cdbecdfafb0a34",
  "4e790dc987501cf2",
  "39563db9a7a617a1",
  "620b01280dba1e97",
  "b88c053b41d86dee",
  "7a6ccd1134d9cf3c",
  "30885ab0ec7a42af",
  "5edbc0c05541f27b",
  "53bed891e975c891",
  "e7d180559b7d15ca",
  "6272c4b7f502533a",
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
