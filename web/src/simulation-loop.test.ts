import { describe, expect, it } from "vitest";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";

describe("fixed-rate simulation clock", () => {
  it("caps catch-up work and drops excess elapsed time", () => {
    const clock = createSimulationClock(20, 4);

    const result = advanceSimulation(clock, 10_000);

    expect(result.ticksToRun).toBe(4);
    expect(result.clock.accumulatorMs).toBe(0);
  });

  it("retains fractional time between wakeups", () => {
    const clock = createSimulationClock(20, 4);

    const first = advanceSimulation(clock, 55);
    const second = advanceSimulation(first.clock, 55);

    expect(first.ticksToRun).toBe(1);
    expect(first.clock.accumulatorMs).toBe(5);
    expect(second.ticksToRun).toBe(1);
    expect(second.clock.accumulatorMs).toBe(10);
  });
});
