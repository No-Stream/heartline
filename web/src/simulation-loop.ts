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
): AdvanceResult {
  const elapsed = Math.max(0, elapsedMs);
  const accumulatedMs = clock.accumulatorMs + elapsed;
  const availableTicks = Math.floor(accumulatedMs / clock.tickDurationMs);
  const ticksToRun = Math.min(availableTicks, clock.maxTicksPerWakeup);
  const accumulatorMs = availableTicks > clock.maxTicksPerWakeup
    ? 0
    : accumulatedMs - ticksToRun * clock.tickDurationMs;
  return {
    clock: { ...clock, accumulatorMs },
    ticksToRun,
  };
}
