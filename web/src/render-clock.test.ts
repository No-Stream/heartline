import { describe, expect, it } from "vitest";
import { RenderClock } from "./render-clock";

type Snapshot = { tickCount: number; position: number };

function position(clock: RenderClock<Snapshot>, now: number): number {
  const frame = clock.frame(now);
  if (!frame) throw new Error("Expected a snapshot");
  return frame.previous.position + (frame.current.position - frame.previous.position) * frame.interpolation;
}

describe("simulation render clock", () => {
  it.each([16, 25, 37])("tracks a steady train with irregular arrivals at %i ms frame intervals", (frameInterval) => {
    const clock = new RenderClock<Snapshot>(30);
    const arrivals = Array.from({ length: 400 }, (_, tickCount) => ({
      tickCount,
      time: tickCount * 1000 / 30 + [0, 9, -4, 11, -7, 3, 6, -5][tickCount % 8],
    })).sort((left, right) => left.time - right.time);
    let arrivalIndex = 0;
    let lastPosition = 0;
    let lastTime = 0;
    for (let now = 0; now <= 10000; now += frameInterval) {
      while (arrivalIndex < arrivals.length && arrivals[arrivalIndex].time <= now) {
        const { tickCount } = arrivals[arrivalIndex++];
        clock.push({ tickCount, position: tickCount });
      }
      const nextPosition = position(clock, now);
      if (now > 1000) {
        const expectedProgress = (now - lastTime) * 30 / 1000;
        expect(nextPosition - lastPosition).toBeGreaterThanOrEqual(expectedProgress * 0.9);
        expect(nextPosition - lastPosition).toBeLessThanOrEqual(expectedProgress * 1.1);
      }
      lastPosition = nextPosition;
      lastTime = now;
    }
  });

  it("recovers after rendering pauses without retaining an unbounded history", () => {
    const clock = new RenderClock<Snapshot>(30);
    clock.push({ tickCount: 0, position: 0 });
    expect(position(clock, 0)).toBe(0);
    for (let tickCount = 1; tickCount <= 300; tickCount++) {
      clock.push({ tickCount, position: tickCount });
    }
    expect(clock.frame(16)!.tick).toBeGreaterThan(0);
    const frame = clock.frame(10000)!;
    expect(frame.tick).toBeGreaterThan(290);
    expect(frame.tick).toBeLessThanOrEqual(300);
    expect(frame.previous.tickCount).toBeLessThanOrEqual(frame.tick);
    expect(frame.current.tickCount).toBeGreaterThanOrEqual(frame.tick);
  });

  it("ignores duplicate and older ticks, brackets the clock, and never moves backwards", () => {
    const clock = new RenderClock<Snapshot>(30);
    expect(clock.frame(0)).toBeUndefined();
    clock.push({ tickCount: 10, position: 10 });
    expect(position(clock, 0)).toBe(10);
    clock.push({ tickCount: 11, position: 11 });
    clock.push({ tickCount: 11, position: 999 });
    clock.push({ tickCount: 9, position: 999 });
    clock.push({ tickCount: 12, position: 12 });
    const frame = clock.frame(20)!;
    expect(frame.previous.tickCount).toBeLessThanOrEqual(frame.tick);
    expect(frame.current.tickCount).toBeGreaterThanOrEqual(frame.tick);
    expect(frame.interpolation).toBeGreaterThanOrEqual(0);
    expect(frame.interpolation).toBeLessThanOrEqual(1);
    const beforeStall = position(clock, 20);
    expect(position(clock, 500)).toBeGreaterThanOrEqual(beforeStall);
    const afterStall = position(clock, 500);
    clock.push({ tickCount: 13, position: 13 });
    expect(position(clock, 516)).toBeGreaterThanOrEqual(afterStall);
  });
});

it("advances interpolation at the selected game speed", () => {
  const clock = new RenderClock<Snapshot>(30);
  clock.push({ tickCount: 0, position: 0 });
  clock.push({ tickCount: 100, position: 100 });
  clock.frame(0);
  clock.setSpeed(4);
  expect(clock.frame(100)!.tick).toBeGreaterThan(9);
});

it("interpolates continuously between batched snapshots at four times speed", () => {
  const clock = new RenderClock<Snapshot>(30);
  let previous = 0;
  for (let now = 0; now <= 2000; now += 10) {
    if (now === 1000) clock.setSpeed(4);
    if (now % 40 === 0) {
      const tickCount = now <= 1000 ? now * 0.03 : 30 + (now - 1000) * 0.12;
      clock.push({ tickCount, position: tickCount });
    }
    const next = position(clock, now);
    if (now > 1400) expect(next - previous).toBeGreaterThan(0.9);
    previous = next;
  }
});

it("builds the larger buffer promptly after increasing speed", () => {
  const clock = new RenderClock<Snapshot>(30);
  for (let now = 0; now <= 1000; now += 10) {
    if (now % 100 === 0) clock.push({ tickCount: now * 0.03, position: now * 0.03 });
    clock.frame(now);
  }
  clock.setSpeed(4);
  let held = false;
  let last = clock.frame(1000)!.tick;
  for (let now = 1010; now <= 1250; now += 10) {
    if (now % 100 === 0) clock.push({ tickCount: 30 + (now - 1000) * 0.12, position: 0 });
    const tick = clock.frame(now)!.tick;
    if (tick === last) held = true;
    if (now === 1010) expect(tick).toBe(last);
    if (now >= 1210) expect(tick - last).toBeGreaterThan(0.9);
    last = tick;
  }
  expect(held).toBe(true);
});

it("freezes the displayed frame while paused and resumes without a time jump", () => {
  const clock = new RenderClock<Snapshot>(30);
  clock.push({ tickCount: 0, position: 0 });
  clock.push({ tickCount: 30, position: 30 });
  clock.frame(0);
  const beforePause = clock.frame(200)!.tick;
  clock.setPaused(true);
  clock.push({ tickCount: 31, position: 31 });
  expect(clock.frame(1200)!.tick).toBe(beforePause);
  clock.setPaused(false);
  expect(clock.frame(1210)!.tick).toBe(beforePause);
  expect(clock.frame(1310)!.tick).toBeGreaterThan(beforePause);
});
