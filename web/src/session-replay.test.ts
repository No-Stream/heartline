import { describe, expect, it } from "vitest";
import {
  firstCheckpointMismatch,
  parseReplaySession,
  prepareReplayCommand,
  replayExpectationFailures,
} from "./session-replay";

const validSession = {
  format: "theme-park-session",
  version: 1,
  build: { gitHash: "abc123", mode: "test" },
  seed: "2026",
  park: "new",
  guestCount: 0,
  commands: [
    { tick: 0, command: { type: "set-surface", x: 10, y: 20, corners: [1, 2, 3, 4], water: -1 } },
    { tick: 10, command: { type: "construction-start", x: 20, y: 20, z: 8, heading: 0 } },
  ],
  checkpoints: [{ tick: 300, hash: "18446744073709551615" }],
  finalTick: 300,
  inputs: [{ action: { type: "pointer" }, time: 12, tick: 0, viewport: { width: 1, height: 1 }, camera: {} }],
  expect: { constructionPiecesAtLeast: 4, chainEnabled: true, pathTilesAtLeast: 6, foodStallsAtLeast: 1 },
  strict: true,
} as const;

describe("session replay helpers", () => {
  it("validates a recorded session and preserves non-command metadata", () => {
    const session = parseReplaySession(validSession);

    expect(session.expect).toEqual(validSession.expect);
    expect(session.inputs).toEqual(validSession.inputs);
    expect(session.build).toEqual(validSession.build);
    expect(session.commands).toHaveLength(2);
  });

  it("rejects unordered commands, out-of-range ticks, and worker-only tick commands", () => {
    expect(() => parseReplaySession({
      ...validSession,
      commands: [...validSession.commands].reverse(),
    })).toThrow(/ordered/);
    expect(() => parseReplaySession({
      ...validSession,
      commands: [{ tick: 301, command: { type: "construction-start" } }],
    })).toThrow(/final tick/);
    expect(() => parseReplaySession({
      ...validSession,
      commands: [{ tick: 0, command: { type: "test-advance", count: 300 } }],
    })).toThrow(/test-advance/);
  });

  it("reconstructs typed arrays required by worker messages", () => {
    const command = prepareReplayCommand(validSession.commands[0]!.command);

    expect(command).toMatchObject({ type: "set-surface", x: 10, y: 20, water: -1 });
    expect(command.corners).toBeInstanceOf(Int16Array);
    expect([...(command.corners as Int16Array)]).toEqual([1, 2, 3, 4]);
  });

  it("preserves launch intensity commands so replay applies the same track target", () => {
    const command = { type: "construction-set-launch-intensity", id: 12, targetSpeed: 52.4 };
    const session = parseReplaySession({
      ...validSession,
      commands: [{ tick: 20, command }],
      checkpoints: [],
      finalTick: 20,
    });

    expect(prepareReplayCommand(session.commands[0]!.command)).toEqual(command);
  });

  it("finds the earliest checkpoint difference, including missing checkpoints", () => {
    expect(firstCheckpointMismatch(
      [{ tick: 300, hash: "11" }, { tick: 600, hash: "22" }],
      [{ tick: 300, hash: "11" }, { tick: 600, hash: "33" }],
    )).toEqual({ tick: 600, expected: "22", actual: "33" });
    expect(firstCheckpointMismatch(
      [{ tick: 300, hash: "11" }, { tick: 600, hash: "22" }],
      [{ tick: 300, hash: "11" }],
    )).toEqual({ tick: 600, expected: "22", actual: undefined });
    expect(firstCheckpointMismatch([{ tick: 300, hash: "11" }], [{ tick: 300, hash: "11" }])).toBeUndefined();
  });

  it("checks recorded outcome expectations against observed worker state", () => {
    const failures = replayExpectationFailures(
      {
        constructionPiecesAtLeast: 4,
        chainEnabled: true,
        pathTilesAtLeast: 6,
        foodStallsAtLeast: 1,
        constructionClosed: true,
        ridersAtLeast: 2,
        revenueAbove: 0,
      },
      {
        constructionPieces: 5,
        chainEnabled: true,
        pathTiles: 8,
        foodStalls: 1,
        constructionClosed: true,
        riders: 2,
        revenue: "100",
      },
    );

    expect(failures).toEqual([]);
    expect(replayExpectationFailures(
      { constructionPiecesAtLeast: 4, chainEnabled: true, pathTilesAtLeast: 6, foodStallsAtLeast: 1 },
      { constructionPieces: 3, chainEnabled: false, pathTiles: 5, foodStalls: 0 },
    )).toHaveLength(4);
  });
});
