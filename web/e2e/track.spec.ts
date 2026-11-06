import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import { browserLaunchOptions } from "./fixtures/browser-launch-options";

let server: PreviewServer;
let fixtureUrl: string;
let outDir: string;

test.use({ launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]) });

test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "track-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: resolve("web/art/atlas"), build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/track.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/track.html`;
});

test.afterAll(async () => {
  await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  await rm(outDir, { recursive: true });
});

for (const rotation of [0, 1, 2, 3] as const) {
  test(`track crossings, path and tunnel at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    const result = await page.evaluate((viewRotation) => window.renderTrackFixture(viewRotation), rotation);
    expect(result.picks).toContain("track:track-1");
    expect(result.picks).toContain("track:track-2");
    await expect(page.locator("canvas")).toHaveScreenshot(`track-crossings-${rotation}.png`, { threshold: 0.1, maxDiffPixelRatio: 0.001 });
  });
}

for (const rotation of [0, 1, 2, 3] as const) {
  test(`worker demo coaster at rotation ${rotation}`, async ({ page }) => {
    test.setTimeout(90_000);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(fixtureUrl);
    const trackPoints = await page.evaluate(() => window.demoTrackPoints());
    expect(trackPoints.length).toBeGreaterThan(0);
    await page.goto("/?test=1");
    await page.waitForFunction(() => window.gameTest?.ready);
    await page.evaluate((viewRotation) => {
      window.gameTest.setCamera({ centerX: 225, centerY: 253, zoom: 0.6, rotation: viewRotation });
    }, rotation);
    await page.waitForFunction(() => window.gameTest.ready);
    const visibleTrack = await page.evaluate((points) => {
      for (const point of points) {
        const screen = window.gameTest.project(point);
        for (let dy = -3; dy <= 3; dy++) for (let dx = -3; dx <= 3; dx++) {
          const pick = window.gameTest.pick(screen.x + dx, screen.y + dy);
          if (pick && "id" in pick && pick.id.startsWith("track-")) return pick.id;
        }
      }
      return null;
    }, trackPoints);
    expect(visibleTrack).toMatch(/^track-/);
    await expect(page.locator("canvas")).toHaveScreenshot(`demo-coaster-${rotation}.png`, { threshold: 0.1, maxDiffPixelRatio: 0.001, timeout: 30_000 });
    expect(errors).toEqual([]);
  });
}

test("interpolates train positions between real worker ticks", async ({ page }) => {
  await page.goto(fixtureUrl);
  const { previous, current, interpolated } = await page.evaluate(() => window.checkTrainMotion());
  expect(current[0]).toBe(previous[0]);
  const displacement = current.slice(1, 4).map((coordinate, axis) => coordinate - previous[axis + 1]);
  const distanceSquared = displacement.reduce((sum, delta) => sum + delta * delta, 0);
  expect(distanceSquared).toBeGreaterThan(0);
  const progress = interpolated.map((pose) => displacement.reduce((sum, delta, axis) =>
    sum + (pose[axis + 1] - previous[axis + 1]) * delta, 0) / distanceSquared);
  for (let index = 0; index < progress.length; index++) {
    expect(progress[index]).toBeCloseTo(index / 4, 2);
    if (index > 0) expect(progress[index]).toBeGreaterThan(progress[index - 1]);
  }
});

test("transfers static rail and tie geometry once and refreshes supports after terrain edits", async ({ page }) => {
  await page.goto(fixtureUrl);
  const result = await page.evaluate(() => window.checkCoasterTransfer());
  expect(result.rails).toBeGreaterThan(0);
  expect(result.rails % 19).toBe(0);
  expect(result.ties).toBeGreaterThan(0);
  expect(result.ties % 13).toBe(0);
  expect(result.supports).toBeGreaterThan(0);
  expect(result.shared).toBe(false);
  expect(result.resent).toBe(false);
  expect(result.refreshedSupports).toBe(true);
});

test("draws and picks banked and inverted riders using the person atlas", async ({ page }) => {
  await page.goto(fixtureUrl);
  const riders = await page.evaluate(() => window.renderRiderOrientations());
  expect(riders.upright.pickable).toBe(true);
  expect(riders.banked.pickable).toBe(true);
  expect(riders.inverted.pickable).toBe(true);
  expect(riders.upright.hiddenByNearerGeometry).toBe(true);
  expect(riders.upright.visibleOverFartherGeometry).toBe(true);
  expect(riders.upright.topMinusBottom.y).toBeLessThan(0);
  expect(Math.abs(riders.banked.topMinusBottom.x)).toBeGreaterThan(2);
  expect(riders.inverted.topMinusBottom.y).toBeGreaterThan(0);
  await expect(page.locator("canvas")).toHaveScreenshot("rider-orientations.png", { threshold: 0.1, maxDiffPixelRatio: 0.001 });
});
