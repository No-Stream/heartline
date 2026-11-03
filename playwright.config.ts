import { defineConfig } from "@playwright/test";

const port = Number(process.env.PLAYWRIGHT_PORT ?? 4173);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./web/e2e",
  snapshotPathTemplate: "{testDir}/{testFilePath}-snapshots/{arg}{-platform}{ext}",
  // Each test boots the WebAssembly simulation and a software-rendered WebGL2 context;
  // one worker per core starved startup and caused timeouts.
  workers: 4,
  use: {
    baseURL,
    launchOptions: { channel: "chrome" },
  },
  projects: [
    {
      name: "feature",
      testIgnore: ["**/*.journey.spec.ts", "**/smoothness.spec.ts"],
      workers: 4,
    },
    {
      name: "journey",
      testMatch: "**/*.journey.spec.ts",
      grep: /@journey/,
      workers: 2,
      use: { viewport: { width: 1280, height: 720 } },
    },
    {
      name: "smoothness",
      testMatch: "**/smoothness.spec.ts",
      workers: 1,
      use: { viewport: { width: 960, height: 540 } },
    },
  ],
  webServer: {
    command: `npm run preview -- --host 127.0.0.1 --port ${port}`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
