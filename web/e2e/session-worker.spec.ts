import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview } from "vite";

test("worker records commands, timed block analysis and checkpoint boundaries", async ({ page }) => {
  const outDir = await mkdtemp(join(tmpdir(), "session-recording-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/session-recording.html") } } };
  await build(config);
  const server = await preview({ ...config, preview: { host: "127.0.0.1", port: 4739, strictPort: true } });
  try {
    const address = server.httpServer?.address();
    if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
    await page.goto(`http://127.0.0.1:${address.port}/e2e/fixtures/session-recording.html`);
    const recording = await page.evaluate(() => window.recordWorkerSession());
    expect(recording.finalTick).toBe(601);
    expect(recording.checkpoints.map(({ tick }) => tick)).toEqual([300, 600]);
    expect(recording.checkpoints.every(({ hash }) => /^\d+$/.test(hash))).toBe(true);
    expect(recording.commands[0]).toEqual({ tick: 0, command: { type: "set-surface", x: 10, y: 20, corners: [1, 2, 3, 4], water: -1 } });
    expect(recording.commands[1].command.type).toBe("construction-start");
    expect(recording.commands.some(({ command }) => command.type === "test-advance-block-analysis")).toBe(true);
  } finally {
    await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
    await rm(outDir, { recursive: true });
  }
});
