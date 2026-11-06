import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import { browserLaunchOptions } from "./fixtures/browser-launch-options";

let server: PreviewServer;
let fixtureUrl: string;
let outDir: string;

test.use({ viewport: { width: 640, height: 480 }, deviceScaleFactor: 1, launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]) });

test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "cliffs-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: "art/atlas", build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/cliffs.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/cliffs.html`;
});

test.afterAll(async () => {
  await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  await rm(outDir, { recursive: true });
});

for (const rotation of [0, 1, 2, 3] as const) {
  test(`renders real atlas cliffs at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    await page.waitForFunction(() => typeof window.renderCliffs === "function");
    await page.evaluate((viewRotation) => window.renderCliffs(viewRotation), rotation);
    await expect(page.locator("canvas")).toHaveScreenshot(`cliffs-rotation-${rotation}.png`, { threshold: 0.1, maxDiffPixelRatio: 0.001 });
  });
}
