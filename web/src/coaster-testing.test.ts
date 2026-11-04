import { describe, expect, it } from "vitest";
import { buildProximityMarkers, buildTestingRibbon, formatProximityEvent, formatSample, formatTestingNote, proximityContribution, summarizeRideHeights, type ProximityEvent, type TestingSample } from "./coaster-testing";
import type { WorldView } from "./render/scene";

const samples: TestingSample[] = [
  { distance: 0, x: 2, y: 3, z: 1, speed: 4, verticalG: 1, lateralG: 0.2, longitudinalG: 0, airtime: false },
  { distance: 1, x: 3, y: 3, z: 1, speed: 12, verticalG: -0.2, lateralG: 0.5, longitudinalG: 0.1, airtime: true },
  { distance: 2, x: 4, y: 3, z: 1, speed: 20, verticalG: 3, lateralG: 1, longitudinalG: -0.1, airtime: false },
];
const camera = { centerX: 3, centerY: 3, zoom: 1, rotation: 0 as const };
const viewport = { width: 800, height: 500 };
const style = { steel: "#e04730", train: "#1f85ad", wood: "#805030", ink: "#211519", lineWeight: 1 };
const proximityEvents: ProximityEvent[] = [
  { kind: "nearMiss", pieceIndex: 13, distance: 14, x: 2, y: 3, z: 1, strength: 0.8 },
  { kind: "dueling", pieceIndex: 7, distance: 8, x: 3, y: 3, z: 1, strength: 0.4 },
];

describe("coaster testing overlay", () => {
  it("describes proximity events and calculates their excitement contribution", () => {
    expect(formatProximityEvent(proximityEvents[0])).toBe("Near-miss with the coaster's supports at piece 14, very close");
    expect(formatProximityEvent(proximityEvents[1])).toBe("Dueling trains at piece 8, close");
    expect(proximityContribution(proximityEvents)).toBeCloseTo(8 * 1.2 / 5.2);
  });

  it("places pickable, theme-coloured markers at each event location", () => {
    const markers = buildProximityMarkers(proximityEvents, camera, viewport, style);
    expect(markers).toHaveLength(4);
    expect(markers.map(({ id }) => id)).toEqual(["proximity-0", "proximity-0", "proximity-1", "proximity-1"]);
    expect(markers[0].color).toEqual([33 / 255, 21 / 255, 25 / 255, 1]);
    expect(markers[1].color).toEqual([224 / 255, 71 / 255, 48 / 255, 1]);
    expect(markers[2].color).toEqual([33 / 255, 21 / 255, 25 / 255, 1]);
    expect(markers[3].color).toEqual([31 / 255, 133 / 255, 173 / 255, 1]);
    const otherKinds = ["headchopper", "tunnel", "terrainHugging", "futureEvent"].map((kind) => ({ ...proximityEvents[0], kind }));
    expect(buildProximityMarkers(otherKinds, camera, viewport, style).filter((_, index) => index % 2 === 1).map(({ color }) => color)).toEqual([
      [128 / 255, 80 / 255, 48 / 255, 1], [33 / 255, 21 / 255, 25 / 255, 1],
      [128 / 255, 80 / 255, 48 / 255, 1], [33 / 255, 21 / 255, 25 / 255, 1],
    ]);
    expect(formatProximityEvent({ ...proximityEvents[0], kind: "futureEvent" })).not.toContain("futureEvent");
  });
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
    expect(formatSample(samples[1], "imperial")).toContain("26.8 mph");
    expect(formatSample(samples[1], "metric")).toContain("43.2 km/h");
    expect(formatSample(samples[1])).toContain("−0.2 g");
    expect(formatSample(samples[1])).toContain("Airtime: yes");
  });

  it("reports sampled maximum height above terrain and the largest downhill drop", () => {
    const world = {
      dimensions: new Uint16Array([2, 2]),
      surfaces: new Int32Array([
        2, 2, 2, 2, 0, 0, 2, 2, 2, 2, 0, 0,
        2, 2, 2, 2, 0, 0, 2, 2, 2, 2, 0, 0,
      ]),
      paths: new Int32Array(),
    } as WorldView;
    const reportSamples: TestingSample[] = [
      { ...samples[0], x: 0.25, y: 0.25, z: 3 },
      { ...samples[1], x: 0.5, y: 0.25, z: 8 },
      { ...samples[2], x: 0.75, y: 0.25, z: 4 },
    ];

    expect(summarizeRideHeights(reportSamples, world)).toEqual({ maximumHeightMetres: 30, biggestDropMetres: 16 });
    expect(summarizeRideHeights([
      { ...reportSamples[1], distance: 0 },
      { ...reportSamples[2], distance: 3 },
    ], world)?.biggestDropMetres).toBe(0);
    expect(summarizeRideHeights([], world)).toBeNull();
  });

  it("formats measured values in ride testing notes using the selected units", () => {
    expect(formatTestingNote(
      "The train could not finish the circuit after travelling 12.0 m. Add a lift or launch, or shorten the circuit.",
      "imperial",
    )).toBe("The train could not finish the circuit after travelling 39.4 ft. Add a lift or launch, or shorten the circuit.");
    expect(formatTestingNote(
      "Riders spend at least 8 m in a row under high forces. Add gentler sections between intense elements.",
      "metric",
    )).toBe("Riders spend at least 8 m in a row under high forces. Add gentler sections between intense elements.");
    expect(formatTestingNote(
      "This layout has at least 100 m in a row below 8 m/s. A taller first drop or a faster section could make it feel livelier.",
      "imperial",
    )).toBe("This layout has at least 328.1 ft in a row below 17.9 mph. A taller first drop or a faster section could make it feel livelier.");
  });
});
