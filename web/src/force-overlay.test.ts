import { describe, expect, it } from "vitest";
import { forceLabelText, forceReadoutLines, limitBand, limitCloseness, nearestSampleIndex, placeForceLabels, rateForce, rateVertical, selectForceLabels, type ForceLimits } from "./force-overlay";
import type { TestingSample } from "./generated/TestingSample";

const steel: ForceLimits = { minimumVerticalG: -1.5, maximumVerticalG: 5, maximumLateralG: 1.5, maximumLongitudinalG: 1.5 };
const sample = (distance: number, forces: Partial<TestingSample> = {}): TestingSample => ({
  distance, x: distance, y: 0, z: 1, speed: 10, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, ...forces,
});

describe("limit closeness", () => {
  it("scales each axis by its own comfortable limit and keeps the worst", () => {
    expect(limitCloseness(sample(0, { verticalG: -1.2 }), steel)).toBeCloseTo(0.8);
    expect(limitCloseness(sample(0, { verticalG: 4 }), steel)).toBeCloseTo(0.8);
    expect(limitCloseness(sample(0, { verticalG: 1, lateralG: -1.2 }), steel)).toBeCloseTo(0.8);
    expect(limitCloseness(sample(0, { verticalG: 1, longitudinalG: 3 }), steel)).toBeCloseTo(2);
    expect(limitCloseness(sample(0), steel)).toBeCloseTo(0.2);
  });
  it("bands the closeness into comfortable, near and past the limit", () => {
    expect(limitBand(0.5)).toBe("comfortable");
    expect(limitBand(0.75)).toBe("near");
    expect(limitBand(1)).toBe("near");
    expect(limitBand(1.01)).toBe("past");
  });
  it("rejects limits that cannot normalise forces", () => {
    expect(() => limitCloseness(sample(0), { ...steel, minimumVerticalG: 0.5 })).toThrow("Invalid comfortable force limits");
  });
});

describe("force ratings", () => {
  it("rates airtime from weak through great to too much and dangerous", () => {
    expect(rateForce("airtime", 0, steel)).toBe("weak");
    expect(rateForce("airtime", -0.5, steel)).toBe("good");
    expect(rateForce("airtime", -1.1, steel)).toBe("great");
    expect(rateForce("airtime", -1.5, steel)).toBe("great");
    expect(rateForce("airtime", -1.6, steel)).toBe("too much");
    expect(rateForce("airtime", -2.1, steel)).toBe("dangerous");
  });
  it("judges airtime against each model's own floor", () => {
    expect(rateForce("airtime", -1.1, { ...steel, minimumVerticalG: -1 })).toBe("too much");
  });
  it("rates positive vertical and lateral force relative to the limits", () => {
    expect(rateForce("vertical", 1.5, steel)).toBe("weak");
    expect(rateForce("vertical", 3, steel)).toBe("good");
    expect(rateForce("vertical", 4.5, steel)).toBe("great");
    expect(rateForce("vertical", 5.5, steel)).toBe("too much");
    expect(rateForce("lateral", -0.4, steel)).toBe("fine");
    expect(rateForce("lateral", -1.2, steel)).toBe("strong");
    expect(rateForce("lateral", 1.8, steel)).toBe("too much");
    expect(rateForce("longitudinal", 2.5, steel)).toBe("dangerous");
  });
  it("treats low vertical force as airtime", () => {
    expect(rateVertical(0.1, steel)).toEqual({ axis: "airtime", rating: "weak" });
    expect(rateVertical(3.2, steel)).toEqual({ axis: "vertical", rating: "good" });
  });
});

describe("force labels", () => {
  it("labels the notable peaks of the recent track only", () => {
    const samples = [
      sample(0, { speed: 25, verticalG: 3.5 }),
      ...Array.from({ length: 40 }, (_, index) => sample(index + 1, { speed: 8 })),
      sample(41, { speed: 12, verticalG: -1.1, airtime: true }),
      sample(42, { speed: 14, verticalG: 3.2, lateralG: 0.9 }),
    ];
    const labels = selectForceLabels(samples, steel);
    expect(labels.map((label) => label.kind)).toEqual(["topSpeed", "peakVertical", "airtime", "peakLateral"]);
    expect(labels.every((label) => label.recent && !label.pastLimit)).toBe(true);
    expect(labels.find((label) => label.kind === "topSpeed")?.sampleIndex).toBe(42);
    expect(forceLabelText(labels[2], samples[41], steel, "metric")).toBe("−1.1 g · great airtime");
    expect(forceLabelText(labels[0], samples[42], steel, "metric")).toBe("50 km/h");
    expect(forceLabelText(labels[0], samples[42], steel, "imperial")).toBe("31 mph");
  });
  it("leaves out gentle forces", () => {
    const labels = selectForceLabels([sample(0), sample(1, { verticalG: 1.4, lateralG: 0.3 })], steel);
    expect(labels.map((label) => label.kind)).toEqual(["topSpeed"]);
  });
  it("labels each stretch past a limit anywhere on the track, worst first, and fades the old ones", () => {
    const samples = [
      sample(0, { verticalG: 5.5 }), sample(1, { verticalG: 6.5 }), sample(2),
      ...Array.from({ length: 40 }, (_, index) => sample(index + 3)),
      sample(43, { verticalG: -1.8 }), sample(44, { verticalG: 5.2 }),
    ];
    const labels = selectForceLabels(samples, steel);
    const pastLimit = labels.filter((label) => label.pastLimit);
    expect(pastLimit.map((label) => [label.kind, label.sampleIndex, label.recent])).toEqual([
      ["peakVertical", 1, false], ["airtime", 43, true], ["peakVertical", 44, true],
    ]);
    expect(labels.filter((label) => label.kind === "peakVertical")).toHaveLength(2);
    expect(forceLabelText(pastLimit[1], samples[43], steel, "metric")).toBe("−1.8 g · too much airtime");
  });
  it("shows every value at a point with its rating", () => {
    expect(forceReadoutLines(sample(0, { speed: 20, verticalG: -0.6, lateralG: 1.2, longitudinalG: -0.2 }), steel, "metric")).toEqual([
      { name: "Speed", value: "72 km/h" },
      { name: "Vertical", value: "−0.6 g · good airtime" },
      { name: "Lateral", value: "1.2 g · strong" },
      { name: "Longitudinal", value: "−0.2 g · fine" },
    ]);
  });
});

describe("label placement", () => {
  const viewport = { width: 400, height: 300 };
  it("puts a label above its anchor and moves later labels out of the way", () => {
    const boxes = placeForceLabels([
      { anchor: { x: 100, y: 100 }, width: 60, height: 16 },
      { anchor: { x: 102, y: 100 }, width: 60, height: 16 },
    ], viewport, [], []);
    expect(boxes[0]).toEqual({ left: 70, top: 75, width: 60, height: 16 });
    expect(boxes[1]?.top).toBeGreaterThan(100);
  });
  it("avoids the drawn track and obstacles, and drops a label that cannot fit", () => {
    const [aboveBlocked] = placeForceLabels([{ anchor: { x: 100, y: 100 }, width: 60, height: 16 }], viewport, [], [{ x: 100, y: 85 }]);
    expect(aboveBlocked?.top).toBe(109);
    const crowded = placeForceLabels([{ anchor: { x: 100, y: 100 }, width: 60, height: 16 }], viewport, [{ left: 0, top: 0, width: 400, height: 300 }], []);
    expect(crowded).toEqual([null]);
  });
  it("finds the sample nearest the pointer within reach", () => {
    const points = [{ x: 0, y: 0 }, { x: 20, y: 0 }, { x: 40, y: 0 }];
    expect(nearestSampleIndex(points, { x: 23, y: 4 }, 14)).toBe(1);
    expect(nearestSampleIndex(points, { x: 23, y: 40 }, 14)).toBeNull();
  });
});
