/**
 * Session files use format "theme-park-session", version 1. The exported file
 * combines build { gitHash, mode } and inputs [{ action, time, tick, viewport,
 * camera }] with this worker recording. Input time is performance.now() in ms,
 * viewport uses backing pixels, and camera is the CSS-zoom camera before the
 * action. Seed is a decimal u64 string; park and guestCount configure the run.
 * Commands are ordered and stamped with the tick before application; typed
 * arrays in commands are JSON arrays. Checkpoints contain decimal u64 hashes
 * after their tick, and finalTick is the simulation tick at export. An optional
 * expect block holds replay assertions; strict makes hash mismatches fail.
 * Replay initializes with manualBlockAnalysis enabled and applies recorded
 * test-advance-block-analysis and test-advance-block-placement commands in
 * order; set-surface corners must be
 * reconstructed as an Int16Array before posting the command to the worker.
 */
export interface WorkerRecording {
  seed: string;
  park: "new" | "demo";
  guestCount: number;
  commands: { tick: number; command: { type: string } }[];
  checkpoints: { tick: number; hash: string }[];
  finalTick: number;
}

export class SessionRecorder {
  private readonly commands: WorkerRecording["commands"] = [];
  private readonly checkpoints: WorkerRecording["checkpoints"] = [];

  constructor(private readonly configuration: Pick<WorkerRecording, "seed" | "park" | "guestCount">) {}

  recordCommand<Command extends { type: string }>(tick: number, command: Command): void {
    if (this.commands.length >= 100_000) throw new Error("Session command limit of 100000 reached");
    const json = JSON.stringify(command, (_key, value: unknown) =>
      ArrayBuffer.isView(value) ? Array.from(value as unknown as ArrayLike<number>) : value,
    );
    this.commands.push({ tick, command: JSON.parse(json) as { type: string } });
  }

  recordCheckpoint(tick: number, hash: bigint): void {
    this.checkpoints.push({ tick, hash: hash.toString() });
  }

  snapshot(finalTick: number): WorkerRecording {
    return structuredClone({ ...this.configuration, commands: this.commands, checkpoints: this.checkpoints, finalTick });
  }
}
