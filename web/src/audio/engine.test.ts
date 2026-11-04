import { expect, it } from "vitest";
import { scheduleClock } from "./engine";

it("uses the audio clock and a 150 ms scheduling horizon", () => {
  expect(scheduleClock(3.25)).toEqual({ now: 3.25, until: 3.4 });
});

it("schedules periodic events through the horizon without repeating them", async () => {
  const { schedulePeriodic } = await import("./engine");
  const first = schedulePeriodic(1, 5, scheduleClock(1));
  expect(first.events).toEqual([1]);
  expect(first.nextTime).toBeCloseTo(1.2);
  const next = schedulePeriodic(first.nextTime, 5, scheduleClock(1.1));
  expect(next.events[0]).toBeCloseTo(1.2);
  expect(next.nextTime).toBeCloseTo(1.4);
});
