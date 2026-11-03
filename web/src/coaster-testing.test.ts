import { describe, expect, it } from "vitest";
import { buildTestingRibbon, formatSample, type TestingSample } from "./coaster-testing";

const samples: TestingSample[] = [
  { distance: 0, x: 2, y: 3, z: 1, speed: 4, verticalG: 1, lateralG: 0.2, longitudinalG: 0, airtime: false },
  { distance: 1, x: 3, y: 3, z: 1, speed: 12, verticalG: -0.2, lateralG: 0.5, longitudinalG: 0.1, airtime: true },
  { distance: 2, x: 4, y: 3, z: 1, speed: 20, verticalG: 3, lateralG: 1, longitudinalG: -0.1, airtime: false },
];
const camera = { centerX: 3, centerY: 3, zoom: 1, rotation: 0 as const };
const viewport = { width: 800, height: 500 };
const style = { steel: "#e04730", train: "#1f85ad", wood: "#805030", ink: "#211519", lineWeight: 1 };

describe("coaster testing overlay", () => {
  it("builds pickable, measurement-coloured segments in every camera rotation", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      const quads = buildTestingRibbon(samples, "speed", { ...camera, rotation }, viewport, style);
      expect(quads).toHaveLength(2);
      expect(quads.map((quad) => quad.id)).toEqual(["testing-0", "testing-1"]);
      expect(quads[0].color).not.toEqual(quads[1].color);
      const alternate = buildTestingRibbon(samples, "speed", { ...camera, rotation }, viewport, { ...style, train: "#000000" });
      expect(alternate[0].color).not.toEqual(quads[0].color);
      expect(quads.flatMap((quad) => quad.vertices).every((vertex) => Number.isFinite(vertex.x) && Number.isFinite(vertex.depth))).toBe(true);
    }
  });

  it("does not connect unrecorded parts of the circuit", () => {
    expect(buildTestingRibbon([samples[0], { ...samples[2], distance: 8 }], "speed", camera, viewport, style)).toHaveLength(0);
  });

  it("skips stationary samples and describes forces and airtime", () => {
    expect(buildTestingRibbon([samples[0], samples[0]], "verticalG", camera, viewport, style)).toHaveLength(0);
    expect(formatSample(samples[1])).toContain("12.0 m/s");
    expect(formatSample(samples[1])).toContain("−0.2 g");
    expect(formatSample(samples[1])).toContain("Airtime: yes");
  });
});
