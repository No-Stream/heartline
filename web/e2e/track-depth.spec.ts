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
  outDir = await mkdtemp(join(tmpdir(), "track-depth-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/track-depth.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/track-depth.html`;
});

test.afterAll(async () => {
  if (server) await new Promise<void>((done, reject) => server.httpServer.close((error) => error ? reject(error) : done()));
  if (outDir) await rm(outDir, { recursive: true });
});

for (const rotation of [0, 1, 2, 3] as const) {
  test(`track depth changes along a crossing and resolves path and tunnel visibility at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    const result = await page.evaluate((view) => window.trackDepthCases(view), rotation);
    expect(result.trackCrossing).toEqual(["track-2", "track-1"]);
    expect(result.flatDepthCrossing).toEqual(["track-2", "track-2"]);
    expect(result.path).toEqual(["track-3", "path"]);
    expect(result.tunnel).toEqual(["terrain", "track-4"]);
  });
}
