import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";

let server: PreviewServer;
let fixtureUrl: string;
let outDir: string;

test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "worker-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/worker.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/worker.html`;
});

test.afterAll(async () => {
  await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  await rm(outDir, { recursive: true });
});

test("worker sends the initial world and sends changes once", async ({ page }) => {
  await page.goto(fixtureUrl);
  expect(await page.evaluate(() => window.checkWorldEdit())).toEqual({
    initial: [8, 8, 8, 8, 0, 0], changed: [1, 2, 3, 4, 1, -1], worldMessages: 2,
  });
});
