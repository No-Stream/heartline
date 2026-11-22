import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { parseReplaySession } from "./session-replay";
import { replaySessionIntoWorker } from "./session-replay-worker";

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
    const ridePath = join(temporaryDirectory, "timber-tumbler.ride.json");
    const sessionPath = join(temporaryDirectory, "timber-tumbler.session.json");
    const ride = JSON.parse(
      await readFile(resolve("content/rides/timber-tumbler.ride.json"), "utf8"),
    ) as { trains: number; steps: unknown[] };
    ride.steps.push({
      blockSections: [],
      trainCapacity: 1,
      capacitySearchComplete: false,
      placementSearchComplete: true,
    });
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
    expect(session.commands.map(({ command }) => command.type)).not.toContain("construction-auto-blocks");
    expect(session.commands.map(({ command }) => command.type)).not.toContain("test-advance-block-analysis");
    expect(session.commands.find(({ command }) => command.type === "construction-set-block-sections")?.command)
      .toMatchObject({
        blockSections: [],
        trainCapacity: 1,
        capacitySearchComplete: false,
        placementSearchComplete: true,
      });
  }, 15_000);

  it("waits for explicit block sections before applying a multi-train count", async () => {
    let releaseBlockSections!: () => void;
    let resolveBlockSectionsCommand!: () => void;
    const blockSectionsCommandSeen = new Promise<void>((resolveCommand) => {
      resolveBlockSectionsCommand = resolveCommand;
    });
    class ReplayWorker extends EventTarget {
      readonly commands: string[] = [];

      postMessage(action: { readonly type: string }): void {
        this.commands.push(action.type);
        if (action.type === "construction-set-block-sections") {
          resolveBlockSectionsCommand();
          releaseBlockSections = () => this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-state" } }));
        } else if (action.type === "construction-set-train-count") {
          this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-state" } }));
        } else if (action.type === "export-session") {
          this.dispatchEvent(new MessageEvent("message", { data: { type: "session-export" } }));
        }
      }
    }
    const worker = new ReplayWorker();
    const session = parseReplaySession({
      format: "theme-park-session",
      version: 1,
      seed: "42",
      park: "new",
      guestCount: 0,
      commands: [
        {
          tick: 0,
          command: {
            type: "construction-set-block-sections",
            blockSections: [{ boundaryMetres: 40, brakeStartMetres: 35, brakePieceIndices: [1, 2] }],
            trainCapacity: 2,
            capacitySearchComplete: true,
            placementSearchComplete: true,
          },
        },
        { tick: 0, command: { type: "construction-set-train-count", count: 2 } },
      ],
      checkpoints: [],
      finalTick: 0,
    });

    const replay = replaySessionIntoWorker(
      worker as unknown as Worker,
      session,
      { action: { type: "export-session", requestId: 1 }, replyType: "session-export" },
    );
    await blockSectionsCommandSeen;
    expect(worker.commands).toEqual(["construction-set-block-sections"]);
    releaseBlockSections();
    await replay;

    expect(worker.commands).toEqual([
      "construction-set-block-sections",
      "construction-set-train-count",
      "export-session",
    ]);
  });
});
