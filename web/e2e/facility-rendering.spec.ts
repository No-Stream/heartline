import { expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build, preview, type PreviewServer } from "vite";
import type { Rotation } from "../src/camera";
import type { FacilityKind } from "../src/facilities";

let server: PreviewServer | undefined;
let fixtureUrl: string;
let outDir: string;

test.use({
  viewport: { width: 840, height: 640 },
  deviceScaleFactor: 1,
  launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] },
});

test.beforeAll(async () => {
  outDir = await mkdtemp(join(tmpdir(), "facility-rendering-e2e-"));
  const config = { configFile: false as const, root: "web", publicDir: resolve("web/art/atlas"), build: { outDir, rollupOptions: { input: resolve("web/e2e/fixtures/facility-rendering.html") } } };
  await build(config);
  server = await preview({ ...config, preview: { host: "127.0.0.1", port: 0 } });
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") throw new Error("Vite did not start on a TCP port");
  fixtureUrl = `http://127.0.0.1:${address.port}/e2e/fixtures/facility-rendering.html`;
});

test.afterAll(async () => {
  const activeServer = server;
  if (activeServer) await new Promise<void>((resolveClose, reject) => activeServer.httpServer.close((error) => error ? reject(error) : resolveClose()));
  await rm(outDir, { recursive: true });
});

const facilityKinds: readonly FacilityKind[] = ["entrance", "food_stall", "drink_stall", "restroom", "souvenir", "carousel", "ferris_wheel"];
const rotations: readonly Rotation[] = [0, 1, 2, 3];

for (const kind of facilityKinds) {
  for (const rotation of rotations) {
    test(`${kind} stays visible beside a path, a raised tile and neighboring buildings at rotation ${rotation}`, async ({ page }) => {
      await page.goto(fixtureUrl);
      const result = await page.evaluate(({ facilityKind, cameraRotation }) => window.renderFacilityFixture(facilityKind, cameraRotation), { facilityKind: kind, cameraRotation: rotation });
      expect(result.upperReferenceSamples, `${kind} at rotation ${rotation}: reference coverage`).toBeGreaterThan(20);
      expect(result.upperGroundSamples, `${kind} at rotation ${rotation}: upper target coverage above terrain and paths`).toBeGreaterThanOrEqual(result.upperReferenceSamples * 0.99);
      expect(result.targetSamples, `${kind} at rotation ${rotation}: target coverage in its image bounds`).toBeGreaterThan(20);
      await expect(page.locator("canvas")).toHaveScreenshot(`${kind}-rotation-${rotation}.png`, {
        animations: "disabled",
        threshold: 0.1,
        maxDiffPixelRatio: 0.001,
      });
    });
  }
}
