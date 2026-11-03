export interface SimulationClock {
  readonly tickDurationMs: number;
  readonly maxTicksPerWakeup: number;
  readonly accumulatorMs: number;
}

export interface AdvanceResult {
  readonly clock: SimulationClock;
  readonly ticksToRun: number;
}

export function createSimulationClock(
  tickRate: number,
  maxTicksPerWakeup: number,
): SimulationClock {
  if (tickRate <= 0 || maxTicksPerWakeup <= 0) {
    throw new RangeError("tickRate and maxTicksPerWakeup must be positive");
  }
  return {
    tickDurationMs: 1000 / tickRate,
    maxTicksPerWakeup,
    accumulatorMs: 0,
  };
}

export function advanceSimulation(
  clock: SimulationClock,
  elapsedMs: number,
  speed = 1,
): AdvanceResult {
  if (!Number.isInteger(speed) || speed < 1 || speed > 16) throw new RangeError("Invalid game speed");
  const elapsed = Math.max(0, elapsedMs);
  const accumulatedMs = clock.accumulatorMs + elapsed;
  const availableTicks = Math.floor(accumulatedMs / clock.tickDurationMs);
  const wakeupsToRun = Math.min(availableTicks, clock.maxTicksPerWakeup);
  const ticksToRun = wakeupsToRun * speed;
  const accumulatorMs = availableTicks > clock.maxTicksPerWakeup
    ? 0
    : accumulatedMs - wakeupsToRun * clock.tickDurationMs;
  return {
    clock: { ...clock, accumulatorMs },
    ticksToRun,
  };
}
