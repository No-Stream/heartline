export interface RecordedSessionCommand {
  readonly tick: number;
  readonly command: Record<string, unknown> & { readonly type: string };
}

export interface ReplayExpectations {
  readonly constructionPiecesAtLeast?: number;
  readonly chainEnabled?: boolean;
  readonly pathTilesAtLeast?: number;
  readonly foodStallsAtLeast?: number;
  readonly constructionClosed?: boolean;
  readonly ridersAtLeast?: number;
  readonly revenueAbove?: number;
}

export interface ReplaySession {
  readonly format: "theme-park-session";
  readonly version: 1;
  readonly seed: string;
  readonly park: "new" | "demo";
  readonly guestCount: number;
  readonly commands: RecordedSessionCommand[];
  readonly checkpoints: { readonly tick: number; readonly hash: string }[];
  readonly finalTick: number;
  readonly expect?: ReplayExpectations;
  readonly strict?: boolean;
  readonly [key: string]: unknown;
}

export interface ReplayCheckpointMismatch {
  readonly tick: number;
  readonly expected: string | undefined;
  readonly actual: string | undefined;
}

export interface ReplayObservation {
  readonly constructionPieces?: number;
  readonly chainEnabled?: boolean;
  readonly pathTiles?: number;
  readonly foodStalls?: number;
  /** True only after construction-close changes canClose from true to false. */
  readonly constructionClosed?: boolean;
  readonly riders?: number;
  /** Cumulative guest spending reported by the worker's facilities message. */
  readonly revenue?: string;
}

function record(value: unknown, description: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${description} must be an object`);
  }
  return value as Record<string, unknown>;
}

function nonNegativeInteger(value: unknown, description: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    throw new TypeError(`${description} must be a non-negative integer`);
  }
  return value;
}

function validateExpectations(value: unknown): ReplayExpectations {
  const object = record(value, "Session expect");
  const supportedKeys = new Set([
    "constructionPiecesAtLeast",
    "chainEnabled",
    "pathTilesAtLeast",
    "foodStallsAtLeast",
    "constructionClosed",
    "ridersAtLeast",
    "revenueAbove",
  ]);
  for (const key of Object.keys(object)) {
    if (!supportedKeys.has(key)) throw new TypeError(`Unsupported session expectation: ${key}`);
  }
  for (const key of ["constructionPiecesAtLeast", "pathTilesAtLeast", "foodStallsAtLeast", "ridersAtLeast"] as const) {
    if (object[key] !== undefined) nonNegativeInteger(object[key], `Session expect.${key}`);
  }
  for (const key of ["chainEnabled", "constructionClosed"] as const) {
    if (object[key] !== undefined && typeof object[key] !== "boolean") {
      throw new TypeError(`Session expect.${key} must be a boolean`);
    }
  }
  if (object.revenueAbove !== undefined && (typeof object.revenueAbove !== "number" || !Number.isSafeInteger(object.revenueAbove) || object.revenueAbove < 0)) {
    throw new TypeError("Session expect.revenueAbove must be a non-negative safe integer");
  }
  return object as ReplayExpectations;
}

export function parseReplaySession(value: unknown): ReplaySession {
  const session = record(value, "Session");
  if (session.format !== "theme-park-session" || session.version !== 1) {
    throw new TypeError("Session format or version is unsupported");
  }
  if (typeof session.seed !== "string" || !/^\d+$/.test(session.seed) || BigInt(session.seed) > 18_446_744_073_709_551_615n) {
    throw new TypeError("Session seed must be a decimal u64 string");
  }
  if (session.park !== "new" && session.park !== "demo") throw new TypeError("Session park must be new or demo");
  const guestCount = nonNegativeInteger(session.guestCount, "Session guestCount");
  const finalTick = nonNegativeInteger(session.finalTick, "Session finalTick");
  if (!Array.isArray(session.commands)) throw new TypeError("Session commands must be an array");
  if (!Array.isArray(session.checkpoints)) throw new TypeError("Session checkpoints must be an array");

  let previousCommandTick = -1;
  for (const [index, value] of session.commands.entries()) {
    const commandRecord = record(value, `Session commands[${index}]`);
    const tick = nonNegativeInteger(commandRecord.tick, `Session commands[${index}].tick`);
    const command = record(commandRecord.command, `Session commands[${index}].command`);
    if (typeof command.type !== "string" || command.type.length === 0) {
      throw new TypeError(`Session commands[${index}].command.type must be a non-empty string`);
    }
    if (command.type === "test-advance") throw new TypeError("Session command journal must not contain test-advance");
    if (tick < previousCommandTick) throw new TypeError("Session commands must be ordered by tick");
    if (tick > finalTick) throw new TypeError(`Session command tick ${tick} exceeds final tick ${finalTick}`);
    previousCommandTick = tick;
  }

  let previousCheckpointTick = 0;
  for (const [index, value] of session.checkpoints.entries()) {
    const checkpoint = record(value, `Session checkpoints[${index}]`);
    const tick = nonNegativeInteger(checkpoint.tick, `Session checkpoints[${index}].tick`);
    if (tick === 0 || tick % 300 !== 0 || tick <= previousCheckpointTick || tick > finalTick) {
      throw new TypeError(`Session checkpoints[${index}].tick must be an increasing 300-tick boundary no later than finalTick`);
    }
    if (typeof checkpoint.hash !== "string" || !/^\d+$/.test(checkpoint.hash)) {
      throw new TypeError(`Session checkpoints[${index}].hash must be a decimal string`);
    }
    previousCheckpointTick = tick;
  }

  if (session.strict !== undefined && typeof session.strict !== "boolean") throw new TypeError("Session strict must be a boolean");
  if (session.expect !== undefined) validateExpectations(session.expect);
  return session as ReplaySession;
}

export function prepareReplayCommand(command: Record<string, unknown> & { readonly type: string }): Record<string, unknown> {
  if (command.type !== "set-surface") return { ...command };
  if (!Array.isArray(command.corners) || command.corners.length !== 4 ||
      !command.corners.every((corner) => Number.isInteger(corner) && corner >= -32_768 && corner <= 32_767)) {
    throw new TypeError("Recorded set-surface command must contain four Int16 corners");
  }
  return { ...command, corners: Int16Array.from(command.corners as number[]) };
}

export function firstCheckpointMismatch(
  expected: readonly { readonly tick: number; readonly hash: string }[],
  actual: readonly { readonly tick: number; readonly hash: string }[],
): ReplayCheckpointMismatch | undefined {
  const expectedByTick = new Map(expected.map(({ tick, hash }) => [tick, hash]));
  const actualByTick = new Map(actual.map(({ tick, hash }) => [tick, hash]));
  const ticks = [...new Set([...expectedByTick.keys(), ...actualByTick.keys()])].sort((left, right) => left - right);
  const tick = ticks.find((candidate) => expectedByTick.get(candidate) !== actualByTick.get(candidate));
  if (tick === undefined) return undefined;
  return { tick, expected: expectedByTick.get(tick), actual: actualByTick.get(tick) };
}

export function replayExpectationFailures(
  expectations: ReplayExpectations,
  observation: ReplayObservation,
): string[] {
  const failures: string[] = [];
  if (expectations.constructionPiecesAtLeast !== undefined) {
    if (observation.constructionPieces === undefined) failures.push("construction state was not received");
    else if (observation.constructionPieces < expectations.constructionPiecesAtLeast) {
      failures.push(`construction pieces: expected at least ${expectations.constructionPiecesAtLeast}, observed ${observation.constructionPieces}`);
    }
  }
  if (expectations.chainEnabled !== undefined) {
    if (observation.chainEnabled === undefined) failures.push("construction chain state was not received");
    else if (observation.chainEnabled !== expectations.chainEnabled) {
      failures.push(`chain enabled: expected ${expectations.chainEnabled}, observed ${observation.chainEnabled}`);
    }
  }
  if (expectations.pathTilesAtLeast !== undefined) {
    if (observation.pathTiles === undefined) failures.push("world path state was not received");
    else if (observation.pathTiles < expectations.pathTilesAtLeast) {
      failures.push(`path tiles: expected at least ${expectations.pathTilesAtLeast}, observed ${observation.pathTiles}`);
    }
  }
  if (expectations.foodStallsAtLeast !== undefined) {
    if (observation.foodStalls === undefined) failures.push("facility state was not received");
    else if (observation.foodStalls < expectations.foodStallsAtLeast) {
      failures.push(`food stalls: expected at least ${expectations.foodStallsAtLeast}, observed ${observation.foodStalls}`);
    }
  }
  if (expectations.constructionClosed !== undefined) {
    if (observation.constructionClosed === undefined) failures.push("construction close state was not received");
    else if (observation.constructionClosed !== expectations.constructionClosed) {
      failures.push(`construction close succeeded: expected ${expectations.constructionClosed}, observed ${observation.constructionClosed}`);
    }
  }
  if (expectations.ridersAtLeast !== undefined) {
    if (observation.riders === undefined) failures.push("construction rider state was not received");
    else if (observation.riders < expectations.ridersAtLeast) {
      failures.push(`riders: expected at least ${expectations.ridersAtLeast}, observed ${observation.riders}`);
    }
  }
  if (expectations.revenueAbove !== undefined) {
    if (observation.revenue === undefined) failures.push("guest revenue state was not received");
    else if (!/^\d+$/.test(observation.revenue) || BigInt(observation.revenue) <= BigInt(expectations.revenueAbove)) {
      failures.push(`guest revenue: expected above ${expectations.revenueAbove}, observed ${observation.revenue}`);
    }
  }
  return failures;
}
