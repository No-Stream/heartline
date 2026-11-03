import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(fileURLToPath(new URL("../..", import.meta.url)));
const playwrightCli = resolve(repositoryRoot, "node_modules/@playwright/test/cli.js");
const result = spawnSync(process.execPath, [
  playwrightCli,
  "test",
  "web/e2e/replay.spec.ts",
  "--project=feature",
  "--workers=1",
], {
  cwd: repositoryRoot,
  env: {
    ...process.env,
    PLAYWRIGHT_PORT: process.env.PLAYWRIGHT_PORT ?? "4740",
    REHASH_SESSIONS: "1",
  },
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
