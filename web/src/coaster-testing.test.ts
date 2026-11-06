import { describe, expect, it } from "vitest";
import { buildProximityMarkers, buildTestingRibbon, formatConstructionEstimate, formatProximityEvent, formatRideRating, formatRideStatistics, formatSample, formatTestingNote, overlaySamples, proximityContribution, summarizeRideHeights, type ProximityEvent, type RideStatistics, type TestingSample } from "./coaster-testing";
import type { WorldView } from "./render/scene";
import { LIMIT_BAND_COLORS } from "./force-overlay";

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
const statistics: RideStatistics = {
  ejectorAirtimeSeconds: 2.25,
  floaterAirtimeSeconds: 3.5,
  airtimeMoments: 4,
  strongPositiveGSeconds: 1.2,
  longestStrongPositiveGSeconds: 0.7,
  strongLateralGSeconds: 0.4,
  maxPositiveVerticalG: 4.1,
  maxNegativeVerticalG: -0.8,
  maxLateralG: 1.4,
  maxForwardG: 1.1,
  maxBrakingG: 0.6,
  maxSpeed: 28,
  averageSpeed: 14,
  durationSeconds: 42.5,
  trackLength: 650,
  maxHeight: 32,
  biggestDrop: 25,
  steepestDropAngleDegrees: 52.3,
  inversions: 2,
  invertedSeconds: 3.1,
  drops: 5,
  launches: 1,
  launchPeakSpeed: 24,
};

describe("coaster testing overlay", () => {
  it("formats ride scores with one decimal on the soft ten-point scale", () => {
    expect(formatRideRating({ excitement: 5.14, intensity: 7.06, nausea: 0.34 })).toBe(
      "Excitement: 5.1 · Intensity: 7.1 · Nausea: 0.3",
    );
  });

  it("formats the ride statistics in metric and imperial units", () => {
    const asText = (rows: readonly { label: string; value: string }[]): string[] => rows.map((row) => `${row.label}: ${row.value}`);
    const formatted = formatRideStatistics(statistics, "metric");
    const metric = { airtime: asText(formatted.airtime), forces: asText(formatted.forces), speedAndLayout: asText(formatted.speedAndLayout) };
    const imperial = { speedAndLayout: asText(formatRideStatistics(statistics, "imperial").speedAndLayout) };

    expect(metric.airtime).toEqual(["Ejector airtime: 2.3 s", "Floater airtime: 3.5 s", "Airtime moments: 4"]);
    expect(metric.forces).toContain("Strong positive g: 1.2 s (longest 0.7 s)");
    expect(metric.forces).toContain("Max negative vertical: −0.8 g");
    expect(metric.speedAndLayout).toContain("Max speed: 100.8 km/h");
    expect(metric.speedAndLayout).toContain("Track length: 650 m");
    expect(metric.speedAndLayout).toContain("Biggest drop: 25 m");
    expect(metric.speedAndLayout).toContain("Launches: 1 (peak 86.4 km/h)");
    expect(imperial.speedAndLayout).toContain("Max speed: 62.6 mph");
    expect(imperial.speedAndLayout).toContain("Track length: 2,133 ft");
    expect(imperial.speedAndLayout).toContain("Max height: 105 ft");
    expect(imperial.speedAndLayout).toContain("Launches: 1 (peak 53.7 mph)");
    expect(formatRideStatistics({ ...statistics, launches: 0 }, "metric").speedAndLayout.at(-1)).toEqual({ label: "Launches", value: "0 (peak —)" });
  });

  it("uses measured samples when available and otherwise shows the construction estimate", () => {
    expect(overlaySamples(samples, [samples[0]])).toBe(samples);
    const estimate = [samples[1], samples[2]];
    expect(overlaySamples(null, estimate)).toBe(estimate);
    expect(overlaySamples(null, null)).toEqual([]);
  });
  it("describes the latest estimated forces, limit violations, and a stalled train", () => {
    const limits = { minimumVerticalG: -1, maximumVerticalG: 4, maximumLateralG: 1.5, maximumLongitudinalG: 1.5 };
    const estimate = { samples: [samples[0], { ...samples[1], verticalG: -2.5, lateralG: -2 }], completed: false, status: "stalled" as const, limits, launches: [], lastPlaced: { maximumSpeed: 12, minimumVerticalG: -2.5, maximumVerticalG: 1, maximumAbsoluteLateralG: 2, maximumAbsoluteLongitudinalG: 0.1 } };
    expect(formatConstructionEstimate(estimate, "metric")).toContain("43.2 km/h");
    expect(formatConstructionEstimate(estimate, "metric")).toContain("−2.5–1.0 g");
    expect(formatConstructionEstimate(estimate, "metric")).toContain("vertical and lateral");
    expect(formatConstructionEstimate(estimate, "metric")).toContain("vertical −2.5 g at 1 m (limit −1.0 g)");
    expect(formatConstructionEstimate(estimate, "metric")).toContain("lateral −2.0 g at 1 m (absolute limit 1.5 g)");
    expect(formatConstructionEstimate(estimate, "metric")).toContain("stopped. Last recorded distance: 1 m");
    expect(formatConstructionEstimate({ ...estimate, status: "timedOut" }, "imperial")).toContain("timed out. Last recorded distance: 3 ft");
    expect(formatConstructionEstimate(estimate, "metric")).toContain("Last placed piece");
    expect(formatConstructionEstimate({ ...estimate, samples: [samples[0]], completed: true, status: "reachedEnd" }, "metric")).not.toContain("comfortable limits");
    const laterPeak = { ...samples[2], verticalG: 6, lateralG: 0, longitudinalG: 2 };
    const warning = formatConstructionEstimate({ ...estimate, samples: [...estimate.samples, laterPeak] }, "metric");
    expect(warning).toContain("vertical 6.0 g at 2 m (limit 4.0 g)");
    expect(warning).toContain("longitudinal 2.0 g at 2 m (absolute limit 1.5 g)");
  });
  it("shows launch speed capability and the configured launch's estimated force", () => {
    const estimate = {
      samples, completed: false, status: "stalled" as const,
      limits: { minimumVerticalG: -1, maximumVerticalG: 4, maximumLateralG: 1.5, maximumLongitudinalG: 1.5 },
      lastPlaced: null,
      launches: [{ pieceIndex: 1, maximumSpeed: 20, peakLongitudinalG: 1.3 }, { pieceIndex: 3, maximumSpeed: 30, peakLongitudinalG: null }],
    };
    const text = formatConstructionEstimate(estimate, "metric");
    expect(text).toContain("Launch on piece 2: approximate maximum speed 72 km/h");
    expect(text).toContain("configured launch's estimated peak longitudinal rider force 1.3 g");
    expect(text).toContain("Launch on piece 4: approximate maximum speed 108 km/h");
    expect(text).toContain("estimated force unavailable: no samples on this launch");
  });
  it("describes proximity events and calculates their excitement contribution", () => {
    expect(formatProximityEvent(proximityEvents[0])).toBe("Near-miss with the coaster's supports at piece 14, very close");
    expect(formatProximityEvent(proximityEvents[1])).toBe("Dueling trains at piece 8, close");
    expect(proximityContribution(proximityEvents)).toBeCloseTo(0.8 * 1.2 / 5.2);
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

  it("highlights the ribbon segment nearest a selected score location", () => {
    const quads = buildTestingRibbon(samples, "speed", camera, viewport, style, 1.25);
    const unselectedQuads = buildTestingRibbon(samples, "speed", camera, viewport, style);
    const selected = quads[1];

    expect(quads.map(({ id }) => id)).toEqual(["testing-0", "testing-1"]);
    expect(selected.color).toEqual([33 / 255, 21 / 255, 25 / 255, 1]);
    const segmentWidth = (quad: typeof selected): number => Math.hypot(
      quad.vertices[0].x - quad.vertices[3].x,
      quad.vertices[0].y - quad.vertices[3].y,
    );
    expect(segmentWidth(selected)).toBeGreaterThan(segmentWidth(unselectedQuads[1]));
  });

  it("marks force-limit violations on the corresponding force ribbon", () => {
    const limits = { minimumVerticalG: -1, maximumVerticalG: 4, maximumLateralG: 1.5, maximumLongitudinalG: 1.5 };
    const readings = [{ ...samples[0], verticalG: -2, lateralG: 2 }, samples[1], samples[2]];
    const vertical = buildTestingRibbon(readings, "verticalG", camera, viewport, style, undefined, limits);
    const lateral = buildTestingRibbon(readings, "lateralG", camera, viewport, style, undefined, limits);
    expect(vertical[0].color).toEqual([0.85, 0.1, 0.1, 1]);
    expect(lateral[0].color).toEqual(vertical[0].color);
    expect(vertical[1].color).not.toEqual(vertical[0].color);
    expect(buildTestingRibbon(readings, "speed", camera, viewport, style, undefined, limits)[0].color).not.toEqual(vertical[0].color);
    expect(buildTestingRibbon(readings, "longitudinalG", camera, viewport, style, undefined, limits)[0].color).not.toEqual(vertical[0].color);
  });

  it("colours limit closeness by band and outlines stretches past a limit in ink", () => {
    const limits = { minimumVerticalG: -1.5, maximumVerticalG: 5, maximumLateralG: 1.5, maximumLongitudinalG: 1.5 };
    const at = (distance: number, verticalG: number): TestingSample => ({ ...samples[0], distance, x: 2 + distance, verticalG, lateralG: 0 });
    const quads = buildTestingRibbon([at(0, 1), at(1, 1), at(2, -1.3), at(3, -1.3), at(4, 6)], "limitCloseness", camera, viewport, style, undefined, limits);
    const colorOf = (hex: string): number[] => [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255).concat(1);
    expect(quads.map((quad) => quad.color)).toEqual([
      colorOf(LIMIT_BAND_COLORS.comfortable), colorOf(LIMIT_BAND_COLORS.near), colorOf(LIMIT_BAND_COLORS.near),
      colorOf(style.ink), colorOf(LIMIT_BAND_COLORS.past),
    ]);
    expect(quads[3].id).toBe(quads[4].id);
    expect(() => buildTestingRibbon(samples, "limitCloseness", camera, viewport, style)).toThrow("comfortable force limits");
  });

  it("does not connect unrecorded parts of the circuit", () => {
    expect(buildTestingRibbon([samples[0], { ...samples[2], distance: 8 }], "speed", camera, viewport, style)).toHaveLength(0);
  });

  it("does not draw a ribbon segment across the circuit distance wrap", () => {
    const wrappedSamples = [
      { ...samples[0], distance: 9 },
      { ...samples[1], distance: 0 },
      { ...samples[2], distance: 1 },
    ];

    expect(buildTestingRibbon(wrappedSamples, "speed", camera, viewport, style).map(({ id }) => id))
      .toEqual(["testing-1"]);
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
      "At Loop (200.0 m along the track), vertical force magnitude was 5.0 g, above the 4.0 g limit. High forces exceeded the estimated ASTM F2291 comfort envelope for 2.4 s total during this car's ride. Lower speeds or gentler transitions can improve comfort.",
      "imperial",
    )).toBe("At Loop (656 ft along the track), vertical force magnitude was 5.0 g, above the 4.0 g limit. High forces exceeded the estimated ASTM F2291 comfort envelope for 2.4 s total during this car's ride. Lower speeds or gentler transitions can improve comfort.");
    expect(formatTestingNote(
      "The train could not finish the circuit after travelling 12.0 m. Add a lift or launch, or shorten the circuit.",
      "imperial",
    )).toBe("The train could not finish the circuit after travelling 39 ft. Add a lift or launch, or shorten the circuit.");
    expect(formatTestingNote(
      "Riders spend at least 8 m in a row under high forces. Add gentler sections between intense elements.",
      "metric",
    )).toBe("Riders spend at least 8 m in a row under high forces. Add gentler sections between intense elements.");
    expect(formatTestingNote(
      "This layout has at least 100 m in a row below 8 m/s. A taller first drop or a faster section could make it feel livelier.",
      "imperial",
    )).toBe("This layout has at least 328 ft in a row below 17.9 mph. A taller first drop or a faster section could make it feel livelier.");
    expect(formatTestingNote(
      "The train entered Steel-structure loop at 30.2 m/s, above its rated speed band of 25.2–27.0 m/s.",
      "imperial",
    )).toBe("The train entered Steel-structure loop at 67.6 mph, above its rated speed band of 56.4–60.4 mph.");
    expect(formatTestingNote(
      "The train entered Vertical loop at 20.0 m/s, below its rated speed band of 24.7–27.5 m/s.",
      "metric",
    )).toBe("The train entered Vertical loop at 72 km/h, below its rated speed band of 88.9–99 km/h.");
  });
});
