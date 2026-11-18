import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { parseReplaySession } from "./session-replay";

describe("design tool session export", () => {
  let temporaryDirectory: string | undefined;

  afterEach(async () => {
    if (temporaryDirectory !== undefined) {
      await rm(temporaryDirectory, { recursive: true, force: true });
      temporaryDirectory = undefined;
    }
  });

  it("exports a ride session accepted by the game's replay parser", async () => {
    temporaryDirectory = await mkdtemp(join(tmpdir(), "design-session-replay-"));
    const ridePath = join(temporaryDirectory, "straight.ride.json");
    const sessionPath = join(temporaryDirectory, "straight.session.json");
    const ride = {
      format: "heartline-ride",
      version: 1,
      name: "Session export acceptance ride",
      model: "hybrid",
      station: { x: 241, y: 249, z: 2, heading: "north" },
      carsPerTrain: 4,
      trains: 1,
      steps: [{ place: "Straight" }],
    };
    await writeFile(ridePath, JSON.stringify(ride));

    execFileSync(resolve("tools/design"), [
      "ride",
      ridePath,
      "export",
      "--session",
      sessionPath,
    ], { cwd: process.cwd(), encoding: "utf8" });

    const session = parseReplaySession(JSON.parse(await readFile(sessionPath, "utf8")) as unknown);

    expect(session.commands.length).toBeGreaterThan(0);
    expect(session.commands.some(({ command }) => command.type.startsWith("construction-"))).toBe(true);
  });
});
