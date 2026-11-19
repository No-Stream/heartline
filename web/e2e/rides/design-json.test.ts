import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";
import { runDesignJson } from "./design-json";

const repositoryRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));

test("parses JSON output larger than the previous subprocess buffer", () => {
  const [report] = runDesignJson<{ readonly payload: string }[]>(
    process.execPath,
    [
      "-e",
      "process.stdout.write(JSON.stringify([{ payload: 'x'.repeat(65 * 1024 * 1024) }]))",
    ],
    repositoryRoot,
  );

  expect(report.payload).toHaveLength(65 * 1024 * 1024);
});

test("propagates a native command failure through temporary output cleanup", () => {
  expect(() => runDesignJson(
    process.execPath,
    ["-e", "process.exit(17)"],
    repositoryRoot,
  )).toThrow(/Command failed/);
});
