import { defineConfig } from "vitest/config";

export default defineConfig({
  root: "web",
  base: "./",
  publicDir: "art/atlas",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  test: { include: ["src/**/*.test.ts"] },
});
