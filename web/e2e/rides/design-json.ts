import { execFileSync } from "node:child_process";
import { closeSync, mkdtempSync, openSync, readFileSync, rmdirSync, unlinkSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

export function runDesignJson<T>(executablePath: string, arguments_: readonly string[], cwd: string): T {
  const temporaryDirectory = mkdtempSync(join(tmpdir(), "ride-agreement-"));
  const outputPath = join(temporaryDirectory, "design-output.json");
  let outputFileCreated = false;
  try {
    const outputDescriptor = openSync(outputPath, "w");
    outputFileCreated = true;
    try {
      execFileSync(executablePath, [...arguments_], {
        cwd,
        stdio: ["ignore", outputDescriptor, "inherit"],
      });
    } finally {
      closeSync(outputDescriptor);
    }
    return JSON.parse(readFileSync(outputPath, "utf8")) as T;
  } finally {
    if (outputFileCreated) unlinkSync(outputPath);
    rmdirSync(temporaryDirectory);
  }
}
