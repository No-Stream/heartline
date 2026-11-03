import { defineConfig } from "@playwright/test";

const port = Number(process.env.PLAYWRIGHT_PORT ?? 4173);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./web/e2e",
  // Each test boots the WebAssembly simulation and a software-rendered WebGL2 context;
  // one worker per core starved startup and caused timeouts.
  workers: 4,
  use: {
    baseURL,
    launchOptions: { channel: "chrome" },
  },
  webServer: {
    command: `npm run preview -- --host 127.0.0.1 --port ${port}`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
