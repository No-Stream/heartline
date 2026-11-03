import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, test } from "vitest";
import { replaceDirectoryWithStagedOutput } from "./pipeline/build-atlas.ts";

const temporaryDirectories: string[] = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

test("a failed atlas generation keeps the published atlas intact", async () => {
  const parentDirectory = mkdtempSync(join(tmpdir(), "atlas-build-test-"));
  temporaryDirectories.push(parentDirectory);
  const outputDirectory = join(parentDirectory, "atlas");
  mkdirSync(outputDirectory);
  writeFileSync(join(outputDirectory, "atlas.json"), "previous atlas");
  writeFileSync(join(outputDirectory, "atlas@1x_0.png"), "previous page");

  await expect(replaceDirectoryWithStagedOutput(outputDirectory, async (stagingDirectory) => {
    writeFileSync(join(stagingDirectory, "atlas@1x_0.png"), "partially generated page");
    throw new Error("page rasterization failed");
  })).rejects.toThrow("page rasterization failed");

  expect(readFileSync(join(outputDirectory, "atlas.json"), "utf8")).toBe("previous atlas");
  expect(readFileSync(join(outputDirectory, "atlas@1x_0.png"), "utf8")).toBe("previous page");
  expect(readdirSync(parentDirectory)).toEqual(["atlas"]);
});
