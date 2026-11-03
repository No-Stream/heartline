import { execFileSync } from "node:child_process";
import { defineConfig } from "vitest/config";

export default defineConfig({
  define: { __GIT_HASH__: JSON.stringify(execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim()) },
  root: "web",
  base: "./",
  publicDir: "art/atlas",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  test: { include: ["src/**/*.test.ts"] },
});
