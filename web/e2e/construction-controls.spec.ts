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
  await page.getByRole("button", { name: "Place station" }).focus();
  await page.keyboard.press("Enter");
  await page.getByLabel("Station height above ground (metres)").selectOption("80");
  await page.getByLabel("Next piece").selectOption("turn");
  await page.getByRole("button", { name: "Place piece" }).focus();
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Delete Straight, piece 2" }).focus();
  await page.keyboard.press("Enter");
  expect(await page.evaluate(() => (window as unknown as { constructionActions: unknown[] }).constructionActions)).toEqual([
    { type: "station-placement", enabled: true },
    { type: "station-height", height: 80 },
    { type: "construction-select", piece: "turn" },
    { type: "construction-append" },
    { type: "construction-delete", id: 9 },
  ]);
  await expect(page.getByRole("button", { name: "Close circuit" })).toBeDisabled();
  await page.evaluate(() => (window as unknown as { setConstructionState: (state: object) => void }).setConstructionState({ canAppend: false, message: "Track collides" }));
  await expect(page.getByRole("button", { name: "Place piece" })).toBeDisabled();
  await expect(page.getByRole("status")).toHaveText("Track collides");
});


test("testing panel shows an all-clear for a report without notes", async ({ page }) => {
  await page.goto(fixtureUrl);
  await page.evaluate(() => (window as unknown as { setTestingReport(report: object): void }).setTestingReport({ samples: [], notes: [], rating: { excitement: 20, intensity: 30, nausea: 5 } }));
  await expect(page.locator("[data-notes]")).toHaveText("No concerns were noted during this test.");
  await expect(page.locator("[data-rating]")).toHaveText("Excitement: 20.0 · Intensity: 30.0 · Nausea: 5.0");
});
