import { defineConfig } from "@playwright/test";

if (!process.env.THEME_PARK_BROWSER_TESTS_IN_BUDGET) {
  throw new Error("Run browser tests with `npm run e2e -- <playwright args>` (or a make target): it runs them one at a time inside the shared memory and CPU budget.");
}

const port = Number(process.env.PLAYWRIGHT_PORT ?? 4173);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./web/e2e",
  snapshotPathTemplate: "{testDir}/{testFilePath}-snapshots/{arg}{-platform}{ext}",
  // Each test boots the WebAssembly simulation and a software-rendered WebGL2 context;
  // one worker per core starved startup and caused timeouts. Two workers fit the six compute cores in
  // tools/compute-cpus; each software-rendered browser uses several cores.
  workers: 2,
  use: {
    baseURL,
    launchOptions: { channel: "chrome" },
  },
  projects: [
    {
      name: "feature",
      testIgnore: ["**/*.journey.spec.ts", "**/smoothness.spec.ts"],
      workers: 2,
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
