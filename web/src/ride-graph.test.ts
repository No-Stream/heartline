import { describe, expect, it } from "vitest";
import { buildRideGraph, formatGraphReadout, nearestSampleIndex, sampleTimes } from "./ride-graph";
import { MODEL_FORCE_LIMITS } from "./generated/modelForceLimits";
import type { TestingSample } from "./generated/TestingSample";

const box = { width: 400, height: 200, left: 40, right: 40, top: 10, bottom: 30 };
const limits = MODEL_FORCE_LIMITS["Velocity Viper"];

function sampleAt(distance: number, speed: number, verticalG = 1, lateralG = 0, longitudinalG = 0): TestingSample {
  return { distance, x: distance, y: 0, z: 0, speed, verticalG, lateralG, longitudinalG, airtime: false, forceLimitExceeded: [false, false, false] };
}

describe("sampleTimes", () => {
  it("accumulates one metre per interval at the mean speed of its two samples", () => {
    const times = sampleTimes([sampleAt(0, 10), sampleAt(1, 10), sampleAt(2, 20)]);
    expect(times[0]).toBe(0);
    expect(times[1]).toBeCloseTo(0.1);
    expect(times[2]).toBeCloseTo(0.1 + 1 / 15);
  });

  it("adds no time while the train is stopped and counts a rollback step as one metre", () => {
    const times = sampleTimes([sampleAt(0, 0), sampleAt(1, 0), sampleAt(5, 10), sampleAt(2, 10)]);
    expect(times[1]).toBe(0);
    expect(times[2]).toBeCloseTo(0.8);
    expect(times[3]).toBeCloseTo(0.9);
  });
});

describe("buildRideGraph", () => {
  const samples = [sampleAt(0, 2, 1), sampleAt(1, 10, 3.5, 0.5, 0.2), sampleAt(2, 20, -0.5, -1.5, -0.3), sampleAt(3, 20, 1)];
  const graph = buildRideGraph(samples, limits, box);

  it("maps the ride time onto the plot width, left to right", () => {
    expect(graph.plot).toEqual({ left: 40, right: 360, top: 10, bottom: 170 });
    expect(graph.timeToX(0)).toBe(40);
    expect(graph.timeToX(graph.durationSeconds)).toBe(360);
    expect(graph.timeToX(graph.durationSeconds / 2)).toBeCloseTo(200);
  });

  it("scales forces so higher g is higher on the page and every limit fits", () => {
    expect(graph.gToY(graph.forceRange.maximum)).toBe(graph.plot.top);
    expect(graph.gToY(graph.forceRange.minimum)).toBe(graph.plot.bottom);
    expect(graph.gToY(2)).toBeLessThan(graph.gToY(1));
    expect(graph.forceRange.minimum).toBeLessThanOrEqual(limits.minimumVerticalG);
    expect(graph.forceRange.maximum).toBeGreaterThanOrEqual(limits.maximumVerticalG);
    expect(graph.forceRange.maximum).toBeGreaterThanOrEqual(3.5);
    expect(graph.forceRange.minimum).toBeLessThanOrEqual(-1.5);
  });

  it("draws one line per force axis plus speed with a point per sample", () => {
    for (const path of [graph.paths.verticalG, graph.paths.lateralG, graph.paths.longitudinalG, graph.paths.speed]) {
      expect(path.startsWith("M")).toBe(true);
      expect(path.match(/[ML]/g)).toHaveLength(samples.length);
    }
  });

  it("puts the fastest speed at the top of the secondary axis", () => {
    expect(graph.speedMaximum).toBeGreaterThanOrEqual(20);
    expect(graph.speedToY(0)).toBe(graph.plot.bottom);
    expect(graph.speedToY(graph.speedMaximum)).toBe(graph.plot.top);
  });

  it("draws a band for each force limit and none without limits", () => {
    expect(graph.bands.map((band) => band.axis)).toEqual(["verticalG", "lateralG", "longitudinalG"]);
    const vertical = graph.bands[0];
    expect(vertical.top).toBeCloseTo(graph.gToY(limits.maximumVerticalG));
    expect(vertical.bottom).toBeCloseTo(graph.gToY(limits.minimumVerticalG));
    const lateral = graph.bands[1];
    expect(lateral.top).toBeCloseTo(graph.gToY(limits.maximumLateralG));
    expect(lateral.bottom).toBeCloseTo(graph.gToY(-limits.maximumLateralG));
    expect(buildRideGraph(samples, null, box).bands).toEqual([]);
  });

  it("labels time, force and speed ticks inside the plot", () => {
    for (const tick of [...graph.timeTicks, ...graph.forceTicks, ...graph.speedTicks]) expect(tick.label).not.toBe("");
    expect(graph.forceTicks.some((tick) => tick.value === 0)).toBe(true);
    expect(graph.forceTicks.find((tick) => tick.value < 0)?.label).toMatch(/^−/);
    for (const tick of graph.timeTicks) expect(tick.position).toBeGreaterThanOrEqual(graph.plot.left);
    for (const tick of graph.forceTicks) expect(tick.position).toBeGreaterThanOrEqual(graph.plot.top);
  });

  it("refuses an empty report", () => {
    expect(() => buildRideGraph([], limits, box)).toThrow("no samples");
  });
});

describe("nearestSampleIndex", () => {
  const samples = [sampleAt(0, 5), sampleAt(1, 5), sampleAt(2, 5), sampleAt(3, 20)];
  const graph = buildRideGraph(samples, limits, box);

  it("returns the sample nearest in ride time to a pointer position", () => {
    expect(nearestSampleIndex(graph, graph.timeToX(graph.times[2]) + 0.4)).toBe(2);
    expect(nearestSampleIndex(graph, graph.timeToX(graph.times[1]) - 0.4)).toBe(1);
  });

  it("clamps pointer positions outside the plot to the first and last sample", () => {
    expect(nearestSampleIndex(graph, -50)).toBe(0);
    expect(nearestSampleIndex(graph, 10_000)).toBe(samples.length - 1);
  });
});

describe("formatGraphReadout", () => {
  const sample = sampleAt(412.4, 25, 3.14, -0.44, 0.8);

  it("reports time, speed, the three forces and distance in the chosen units", () => {
    expect(formatGraphReadout(sample, 12.34, "metric")).toBe("12.3 s · Speed 90 km/h · Vertical 3.1 g · Lateral −0.4 g · Forward 0.8 g · 412 m along the track");
    expect(formatGraphReadout(sample, 12.34, "imperial")).toBe("12.3 s · Speed 56 mph · Vertical 3.1 g · Lateral −0.4 g · Forward 0.8 g · 1,353 ft along the track");
  });
});
