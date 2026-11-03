import { expect, test } from "@playwright/test";
import type { GameTest } from "../src/game-test";

test.use({
  viewport: { width: 1100, height: 750 },
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
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

test("picks known terrain and path points at every rotation", async ({ page }) => {
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
      const locations = [
        { x: 240.5, y: 254.5, z: 2 },
        { x: 240.25, y: 256.25, z: 2 },
        { x: 250.5, y: 256.5, z: 3.375 },
      ];
      return locations.map((location) => {
        const screen = window.gameTest.project(location);
        return window.gameTest.pick(screen.x, screen.y);
      });
    });

    expect(picks).toEqual([
      { kind: "terrain", x: 240, y: 254, height: expect.closeTo(8, 8) },
      { kind: "path", x: 240, y: 256, height: expect.closeTo(8, 8) },
      { kind: "path", x: 250, y: 256, height: expect.closeTo(13.5, 1) },
    ]);

    await renderCamera(220, 250, rotation);
    const entrance = await page.evaluate(() => {
      const screen = window.gameTest.project({ x: 220.25, y: 250.35, z: 1.6875 });
      return window.gameTest.pick(screen.x, screen.y);
    });
    expect(entrance, `entrance at rotation ${rotation}`).toEqual({ kind: "path", x: 220, y: 250, height: expect.closeTo(6.75, 1) });
  }
});


test("guest appearances on paths", async ({ page }) => {
  await page.goto("/?test=1");
  await page.waitForFunction(() => window.gameTest?.ready);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 219.5, centerY: 250.5, zoom: 2, rotation: 0 }));
  await page.waitForFunction(() => window.gameTest.ready);
  await expect(page.locator("canvas")).toHaveScreenshot("guests-on-paths.png", {
    animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001,
  });
});

test("a terrain edit repaints the frozen world and updates picking", async ({ page }) => {
  await page.goto("/?test=1");
  await page.waitForFunction(() => window.gameTest?.ready);
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
