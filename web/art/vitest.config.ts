import { defineConfig } from "vitest/config";

export default defineConfig({
  root: "web/art",
  test: { include: ["*.test.ts"] },
});
