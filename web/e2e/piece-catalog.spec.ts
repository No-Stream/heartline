import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { build, preview, type PreviewServer } from "vite";

interface CatalogPiece { name: string; rails: number[]; ties: number[] }
let server: PreviewServer;
let fixtureUrl: string;
let outDir: string;
let pieces: CatalogPiece[];

test.use({ viewport: { width: 1460, height: 1200 }, launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] } });

test.beforeAll(async () => {
  const cargo = join(process.env.HOME!, ".cargo/bin/cargo");
  pieces = JSON.parse(execFileSync(cargo, ["run", "--quiet", "--release", "-p", "theme-park-sim", "--example", "piece_visual_fixture"], { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 })) as CatalogPiece[];
  if (!pieces.length) throw new Error("The coaster model has no pieces");
  for (const name of [
    "TopHat", "HalfLoopUp", "HalfLoopDown", "HalfCorkscrewLeftUp", "HalfCorkscrewRightUp", "HalfCorkscrewLeftDown", "HalfCorkscrewRightDown",
    "LevelToSixtyUp", "SixtyUpToLevel", "LevelToSixtyDown", "SixtyDownToLevel",
    "BankedDropLeft", "BankedDropRight", "TwistedDropLeft", "TwistedDropRight", "BarrelDropLeft", "BarrelDropRight",
  ]) {
    expect(pieces.map((piece) => piece.name)).toContain(name);
  }
  outDir = await mkdtemp(join(tmpdir(), "piece-catalog-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: false as const, build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/piece-catalog.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/piece-catalog.html`;
});

test.afterAll(async () => {
  if (server) await new Promise<void>((resolveClose, reject) => server.httpServer.close((error) => error ? reject(error) : resolveClose()));
  if (outDir) await rm(outDir, { recursive: true });
});

for (const rotation of [0, 1, 2, 3] as const) {
  test(`every allowed coaster piece renders and can be picked at rotation ${rotation}`, async ({ page }) => {
    await page.goto(fixtureUrl);
    const missing = await page.evaluate(({ catalog, view }) => window.renderPieceCatalog(catalog, view), { catalog: pieces, view: rotation });
    expect(missing).toEqual([]);
    await expect(page.locator("canvas")).toHaveScreenshot(`piece-catalog-${rotation}.png`, { threshold: 0.1, maxDiffPixelRatio: 0.001 });
  });
}
