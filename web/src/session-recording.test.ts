import { describe, expect, it } from "vitest";
import { SessionRecorder } from "./session-recording";

describe("SessionRecorder", () => {
  it("records commands in order at their application tick and returns independent snapshots", () => {
    const recorder = new SessionRecorder({ seed: "2026", park: "new", guestCount: 3 });
    const command = { type: "path-build", start: { x: 1, y: 2 } };
    recorder.recordCommand(4, command);
    command.start.x = 9;
    recorder.recordCommand(4, { type: "construction-append" });
    recorder.recordCheckpoint(300, 123n);
    const snapshot = recorder.snapshot(301);
    expect(snapshot).toEqual({ seed: "2026", park: "new", guestCount: 3, commands: [
      { tick: 4, command: { type: "path-build", start: { x: 1, y: 2 } } },
      { tick: 4, command: { type: "construction-append" } },
    ], checkpoints: [{ tick: 300, hash: "123" }], finalTick: 301 });
    snapshot.commands.length = 0;
    expect(recorder.snapshot(301).commands).toHaveLength(2);
  });

  it("fails before applying an additional command when the journal is full", () => {
    const recorder = new SessionRecorder({ seed: "1", park: "demo", guestCount: 0 });
    for (let index = 0; index < 100_000; index++) recorder.recordCommand(0, { type: "command" });
    expect(() => recorder.recordCommand(0, { type: "overflow" })).toThrow(/100000/);
    expect(recorder.snapshot(0).commands).toHaveLength(100_000);
  });
});
