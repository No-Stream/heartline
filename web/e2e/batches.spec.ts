import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import { browserLaunchOptions } from "./fixtures/browser-launch-options";

let server: PreviewServer; let fixtureUrl: string; let outDir: string;
test.use({ launchOptions: browserLaunchOptions(["--use-gl=angle", "--use-angle=swiftshader"]) });
test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "batches-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/batches.html") } } };
  await build(config); server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address(); if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/batches.html`;
});
test.afterAll(async () => {
  await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  await rm(outDir, { recursive: true });
});
for (const rotation of [0, 1, 2, 3] as const) test(`preserves page order and picking IDs at rotation ${rotation}`, async ({ page }) => {
  await page.goto(fixtureUrl);
  const result = await page.evaluate((value) => window.batchFixture(value), rotation);
  expect(result.ids).toEqual(["sprite-0", "sprite-1", "sprite-2"]);
  expect(result.colors).toEqual([[255, 0, 0, 255], [0, 0, 255, 255], [255, 0, 0, 255]]);
});
