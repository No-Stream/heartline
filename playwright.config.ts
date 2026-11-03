import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./web/e2e",
  // Each test boots the WebAssembly simulation and a software-rendered WebGL2 context;
  // one worker per core starved startup and caused timeouts.
  workers: 4,
  use: {
    baseURL: "http://127.0.0.1:4173",
    launchOptions: { channel: "chrome" },
  },
  webServer: {
    command: "npm run preview -- --host 127.0.0.1 --port 4173",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
  },
});
