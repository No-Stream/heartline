import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview } from "vite";
import { browserLaunchOptions } from "./fixtures/browser-launch-options";
import type { GameTest } from "../src/game-test";

test.use({
  viewport: { width: 1100, height: 750 },
  deviceScaleFactor: 1,
  launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]),
});

for (const rotation of [0, 1, 2, 3] as const) {
  test(`world view at rotation ${rotation}`, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/?test=1");
    await page.waitForFunction(() => window.gameTest?.ready);
    await page.evaluate((viewRotation) => {
      const game: GameTest = window.gameTest;
      game.setCamera({ centerX: 238, centerY: 249, zoom: 0.7, rotation: viewRotation });
    }, rotation);
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    expect(errors).toEqual([]);
    await expect(page.locator("canvas")).toHaveScreenshot(`world-rotation-${rotation}.png`, {
      animations: "disabled",
      threshold: 0.1,
      maxDiffPixelRatio: 0.001,
    });
    expect(errors).toEqual([]);
  });
}

test("picks known terrain and path points at every rotation @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await page.waitForFunction(() => window.gameTest?.ready);

  const renderCamera = async (centerX: number, centerY: number, rotation: 0 | 1 | 2 | 3): Promise<void> => {
    await page.evaluate(({ cameraCenterX, cameraCenterY, viewRotation }) => {
      window.gameTest.setCamera({ centerX: cameraCenterX, centerY: cameraCenterY, zoom: 0.7, rotation: viewRotation });
    }, { cameraCenterX: centerX, cameraCenterY: centerY, viewRotation: rotation });
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  };

  for (const rotation of [0, 1, 2, 3] as const) {
    await renderCamera(245, 256, rotation);

    const picks = await page.evaluate(() => {
      // Points the demo coaster's timber bents leave uncovered at every rotation.
      const locations = [
        { x: 241.5, y: 254.5, z: 2 },
        { x: 240.25, y: 256.25, z: 2 },
        { x: 250.5, y: 256.7, z: 3.375 },
      ];
      return locations.map((location) => {
        const screen = window.gameTest.project(location);
        return window.gameTest.pick(screen.x, screen.y);
      });
    });

    expect(picks).toEqual([
      { kind: "terrain", x: 241, y: 254, height: expect.closeTo(8, 8) },
      { kind: "path", x: 240, y: 256, height: expect.closeTo(8, 8) },
      { kind: "path", x: 250, y: 256, height: expect.closeTo(13.5, 1) },
    ]);

    await renderCamera(219, 250, rotation);
    const entrancePath = await page.evaluate(() => {
      // This point on the entrance tile stays visible around the demo coaster supports at every rotation.
      const screen = window.gameTest.project({ x: 218.5, y: 250.65, z: 2 });
      return window.gameTest.pick(screen.x, screen.y);
    });
    expect(entrancePath, `entrance path at rotation ${rotation}`).toEqual({ kind: "path", x: 218, y: 250, height: expect.closeTo(8, 1) });
  }
});


test("guest appearances on paths @slow", async ({ page }) => {
  test.setTimeout(90_000);
  const fixtureDir = await mkdtemp(join(tmpdir(), "guests-e2e-"));
  try {
    const config = { configFile: false as const, root: "web", publicDir: resolve("web/dist"), build: { outDir: join(fixtureDir, "dist"), rollupOptions: { input: resolve("web/e2e/fixtures/guests.html") } } };
    await build(config);
    const server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
    try {
      const address = server.httpServer?.address();
      if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
      await page.goto(`http://127.0.0.1:${address.port}/e2e/fixtures/guests.html`);
      const visibleGuests = await page.evaluate(() => window.renderGuestFixture());
      expect(visibleGuests.length).toBeGreaterThanOrEqual(2);
      await expect(page.locator("canvas")).toHaveScreenshot("guests-on-paths.png", {
        animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001,
      });
    } finally {
      await server.close();
    }
  } finally {
    await rm(fixtureDir, { recursive: true });
  }
});

test("a terrain edit repaints the frozen world and updates picking", async ({ page }) => {
  const runtimeEvents: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") runtimeEvents.push(`console error: ${message.text()}`);
  });
  page.on("pageerror", (error) => runtimeEvents.push(`page error: ${error.message}`));
  page.on("requestfailed", (request) => runtimeEvents.push(`request failed: ${request.url()} (${request.failure()?.errorText ?? "unknown error"})`));
  page.on("worker", (worker) => {
    runtimeEvents.push(`worker started: ${worker.url()}`);
    worker.on("close", () => runtimeEvents.push(`worker closed: ${worker.url()}`));
  });
  await page.goto("/?test=1");
  try {
    await page.waitForFunction(() => window.gameTest?.ready);
  } catch (error) {
    const tickCount = await page.locator("[data-testid='tick-count']").textContent();
    throw new Error(`Game did not become ready; tick count was ${tickCount}. Runtime events: ${runtimeEvents.join("; ") || "none"}`, { cause: error });
  }
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 240, centerY: 254, zoom: 1, rotation: 0 }));
  await page.waitForFunction(() => window.gameTest.ready);
  const initial = await page.evaluate(() => {
    const screen = window.gameTest.project({ x: 240.5, y: 254.5, z: 2 });
    return window.gameTest.pick(screen.x, screen.y);
  });
  expect(initial).toMatchObject({ kind: "terrain", x: 240, y: 254, height: 8 });
  await page.evaluate(() => window.gameTest.setSurface(240, 254, [12, 12, 12, 12]));
  await expect.poll(() => page.evaluate(() => {
    const screen = window.gameTest.project({ x: 240.5, y: 254.5, z: 3 });
    return window.gameTest.pick(screen.x, screen.y);
  })).toMatchObject({ kind: "terrain", x: 240, y: 254, height: 12 });
});
