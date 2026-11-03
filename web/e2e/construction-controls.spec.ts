import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import { expect, test } from "@playwright/test";

let server: PreviewServer;
let outDir: string;
let fixtureUrl: string;
test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "construction-controls-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/construction.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/construction.html`;
});
test.afterAll(async () => {
  if (server) await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  if (outDir) await rm(outDir, { recursive: true });
});

test("construction controls dispatch actions with the keyboard and reflect worker validity", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.getByLabel("Station X (tiles)").fill("12");
  await page.getByLabel("Station Y (tiles)").fill("15");
  await page.getByRole("button", { name: "Start coaster" }).focus();
  await page.keyboard.press("Enter");
  await page.getByLabel("Next piece").selectOption("turn");
  await page.getByRole("button", { name: "Place piece" }).focus();
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Delete Straight, piece 2" }).focus();
  await page.keyboard.press("Enter");
  expect(await page.evaluate(() => (window as unknown as { constructionActions: unknown[] }).constructionActions)).toEqual([
    { type: "construction-start", x: 12, y: 15, z: 12, heading: 0 },
    { type: "construction-select", piece: "turn" },
    { type: "construction-append" },
    { type: "construction-delete", id: 9 },
  ]);
  await expect(page.getByRole("button", { name: "Close circuit" })).toBeDisabled();
  await page.evaluate(() => (window as unknown as { setConstructionState: (state: object) => void }).setConstructionState({ canAppend: false, message: "Track collides" }));
  await expect(page.getByRole("button", { name: "Place piece" })).toBeDisabled();
  await expect(page.getByRole("status")).toHaveText("Track collides");
});
