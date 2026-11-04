import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import type { Rotation } from "../src/camera";

let server: PreviewServer;
let fixtureUrl: string;
let outDir: string;

test.use({ launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] } });

test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "renderer-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/renderer.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/renderer.html`;
});

test.afterAll(async () => {
  await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  await rm(outDir, { recursive: true });
});

for (const rotation of [0, 1, 2, 3] as const satisfies readonly Rotation[]) {
  test(`extra solid and textured geometry shares guest depth and picking at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    const behind = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, false, false, "behind"), rotation);
    expect(behind.pick).toMatchObject({ kind: "sprite", id: "guest" });
    expect(behind.color).toEqual([25, 163, 59, 255]);
    const solid = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, false, false, "solid"), rotation);
    expect(solid.pick).toMatchObject({ kind: "fixture", id: "extra" });
    expect(solid.color).toEqual([0, 0, 255, 255]);
    const textured = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, false, false, "textured"), rotation);
    expect(textured.pick).toMatchObject({ kind: "fixture", id: "extra" });
    expect(textured.color).toEqual([200, 45, 54, 255]);
    const thin = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, false, false, "thin"), rotation);
    expect(thin.pick).toMatchObject({ kind: "fixture", id: "extra", height: 2 });
  });
  test(`WebGL depth and picking on slopes at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    const terrain = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, false, false), rotation);
    expect(terrain.pick).toMatchObject({ kind: "terrain", x: 0, y: 0 });
    expect(terrain.pick!.height).toBeCloseTo(0.5, 1);
    expect(terrain.color).toEqual([25, 163, 59, 255]);

  });

  test(`WebGL picking on stacked paths at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    const upper = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, true, false), rotation);
    const [x, y] = [[2, 2], [2, 1], [1, 1], [1, 2]][rotation];
    expect(upper.pick).toMatchObject({ kind: "path", x, y });
    expect(upper.pick!.height).toBeCloseTo(4.5, 1);
    expect(upper.color).toEqual([200, 45, 54, 255]);

    const lower = await page.evaluate((viewRotation) => window.renderFixture(viewRotation, true, true), rotation);
    const [lowerX, lowerY] = [[1, 1], [1, 2], [2, 2], [2, 1]][rotation];
    expect(lower.pick).toMatchObject({ kind: "path", x: lowerX, y: lowerY });
    expect(lower.pick!.height).toBeCloseTo(0, 1);
    expect(lower.color).toEqual([200, 45, 54, 255]);
  });
}

test("picking is drawn on demand and refreshed after scene and canvas changes", async ({ page }) => {
  await page.goto(fixtureUrl);
  const result = await page.evaluate(() => window.renderFixture(0, false, false, "solid"));
  const [afterRender, afterPick, afterRepeatedPick, afterUpdatedRender, afterUpdatedPick, afterResizedRender, afterResizedPick] = result.pickingPasses;
  expect(afterRender).toBe(0);
  expect(afterPick).toBeGreaterThan(afterRender);
  expect(afterRepeatedPick).toBe(afterPick);
  expect(afterUpdatedRender).toBe(afterPick);
  expect(afterUpdatedPick).toBeGreaterThan(afterUpdatedRender);
  expect(afterResizedRender).toBe(afterUpdatedPick);
  expect(afterResizedPick).toBeGreaterThan(afterResizedRender);
  expect(result.pick).toMatchObject({ kind: "fixture", id: "extra" });
  expect(result.updatedPick).toMatchObject({ kind: "terrain", x: 0, y: 0 });
  expect(result.resizedPick).toMatchObject({ kind: "terrain", x: 0, y: 0 });
});

test("world and picking draws establish their required WebGL state", async ({ page }) => {
  await page.goto(fixtureUrl);
  const normal = await page.evaluate(() => window.renderFixture(0, false, false, "solid"));
  const disturbed = await page.evaluate(() => window.renderFixture(0, false, false, "solid", false, true));
  expect(disturbed.color).toEqual(normal.color);
  expect(disturbed.pick).toEqual(normal.pick);
});

test("region picking matches individual picks, including clipped pixels", async ({ page }) => {
  await page.goto(fixtureUrl);
  for (const extra of ["behind", "solid", "textured", "thin"] as const) {
    const result = await page.evaluate((kind) => window.renderFixture(0, false, false, kind, true), extra);
    expect(result.regionMatches).toBe(true);
    expect(result.clippedRegionMatches).toBe(true);
  }
});
