import { describe, expect, it } from "vitest";
import {
  findingDetails, findingHeadline, findingTitles, formatFindingEffects, formatFindingHeadline, groupScoreFindings, mainFindingCount, splitScoreFindings, type ScoreFinding,
} from "./score-findings";
import type { RideStatisticsReport } from "./generated/RideStatisticsReport";
import type { ScoreBreakdownReport } from "./generated/ScoreBreakdownReport";
import type { ScoreContributionReport } from "./generated/ScoreContributionReport";
import type { ScoreForceDetailReport } from "./generated/ScoreForceDetailReport";

const turn = { pieceIndex: 23, pieceName: "Begin large left turn", distance: 582.1, seconds: 85.5 };
const loop = { pieceIndex: 21, pieceName: "Vertical loop", distance: 446, seconds: 80.2 };
const descent = { pieceIndex: 20, pieceName: "End gentle descent", distance: 405.9, seconds: 79.3 };

// Numbers from the lift-circuit ride test in web/e2e/construction.spec.ts.
const envelopeDetail: ScoreForceDetailReport = { axis: "lateral", measuredValue: 4.76, limit: 3, unit: "g", durationSeconds: 1.75, location: turn, carIndex: 0 };
const excessiveDetails: ScoreForceDetailReport[] = [
  { axis: "vertical", measuredValue: 5.42, limit: 4.5, unit: "g", durationSeconds: 0.2, location: descent, carIndex: 5, dangerousSeconds: 0 },
  { axis: "lateral", measuredValue: 4.76, limit: 1.5, unit: "g", durationSeconds: 3.52, location: turn, carIndex: 0, dangerousSeconds: 3.31 },
];
const positiveDetails: ScoreForceDetailReport[] = [
  { axis: "verticalGood", measuredValue: 3.6, limit: 2, unit: "g", durationSeconds: 2.31, location: loop, weight: 0.55, carIndex: 5 },
  { axis: "verticalGreat", measuredValue: 4.5, limit: 3.6, unit: "g", durationSeconds: 1.59, location: loop, weight: 1, carIndex: 0 },
];
const lateralDetails: ScoreForceDetailReport[] = [
  { axis: "lateralStrong", measuredValue: 4.76, limit: 0.75, unit: "g", durationSeconds: 3.84, location: turn, weight: 1, carIndex: 0 },
];
const envelope = (value: number): ScoreContributionReport => ({ term: "envelope", value, measure: 0.42, unit: "g-s", location: turn, forceDetail: envelopeDetail });
const excessive = (value: number): ScoreContributionReport => ({ term: "excessiveForces", value, measure: 7.09, unit: "s", forceDetails: excessiveDetails });
const breakdown: ScoreBreakdownReport = {
  excitement: [
    { term: "positiveForces", value: 0.79, measure: 3.15, unit: "s", forceDetails: positiveDetails },
    { term: "laterals", value: 0.6, measure: 3.84, unit: "s", forceDetails: lateralDetails },
    excessive(-0.68),
    { term: "inversions", value: 0.8, measure: 0.9, unit: "" },
    { term: "airtime", value: 0, measure: 0, unit: "s" },
    { term: "excitementFloor", value: 0, measure: 0, unit: "" },
    envelope(-1.02),
  ],
  intensity: [
    { term: "peakForce", value: 3.6, measure: 1.59, unit: "", location: turn, forceDetails: [{ axis: "peakLateral", measuredValue: 4.76, limit: 3, unit: "g", location: turn, carIndex: 0 }] },
    excessive(0.97),
    { term: "inversions", value: 0.2, measure: 1, unit: "" },
    envelope(0.34),
  ],
  nausea: [
    { term: "laterals", value: 1.8, measure: 3.84, unit: "s", forceDetails: lateralDetails },
    { term: "greyOut", value: 0.54, measure: 3.15, unit: "s", forceDetails: positiveDetails },
    excessive(1.46),
    { term: "timeInverted", value: 0.03, measure: 1.7, unit: "s" },
    envelope(0.8504),
  ],
};
const statistics = { maxSpeed: 30.4, inversions: 1 } as RideStatisticsReport;
const findings = groupScoreFindings(breakdown);
const finding = (key: string): ScoreFinding => findings.find((candidate) => candidate.key === key)!;
const headline = (key: string, units: "metric" | "imperial" = "metric"): string => formatFindingHeadline(findingHeadline(finding(key), statistics, units));

describe("score findings", () => {
  it("shows each scoring term once with its effect on every score, biggest total effect first", () => {
    expect(findings.map((candidate) => candidate.key)).toEqual([
      "peakForce", "excessiveForces", "laterals", "envelope", "positiveForces", "inversions", "timeInverted", "airtime", "excitementFloor",
    ]);
    expect(formatFindingEffects(finding("envelope"))).toBe("Excitement −1.0 · Intensity +0.3 · Nausea +0.9");
    expect(formatFindingEffects(finding("laterals"))).toBe("Excitement +0.6 · Nausea +1.8");
    // Nausea's strong positive forces read the same seconds as excitement's positive forces, so they are one finding.
    expect(formatFindingEffects(finding("positiveForces"))).toBe("Excitement +0.8 · Nausea +0.5");
    expect(() => groupScoreFindings({ ...breakdown, nausea: [{ term: "mystery", value: 1, measure: 1, unit: "" }] })).toThrow("Unknown coaster score term: mystery");
  });

  it("folds small findings behind more detail and lists the ones that changed nothing", () => {
    const { main, more, unchanged } = splitScoreFindings(findings);
    expect(main).toHaveLength(mainFindingCount);
    expect(more.map((candidate) => candidate.key)).toEqual(["inversions"]);
    // Time inverted moves nausea by +0.03, which prints as +0.0, so it counts as no effect.
    expect(unchanged.map((candidate) => candidate.key)).toEqual(["timeInverted", "airtime"]);
  });

  it("writes one short plain line per finding with the force, value, limit, duration and place", () => {
    expect(headline("envelope")).toBe("Past the ASTM comfort envelope at Begin large left turn: sideways force 4.8 g for 1.8 s (limit 3.0 g)");
    expect(headline("excessiveForces")).toBe("Over the train's force limit at Begin large left turn: sideways force 4.8 g for 3.5 s (limit 1.5 g), plus 1 other force");
    expect(headline("laterals")).toBe("Strong sideways force at Begin large left turn: up to 4.8 g, 3.8 s above 0.8 g");
    expect(headline("positiveForces")).toBe("Pushed into the seat at Vertical loop: up to 4.5 g, 1.6 s above 3.6 g");
    expect(headline("peakForce")).toBe("Strongest force at Begin large left turn: sideways 4.8 g (scoring reference 3.0 g)");
    expect(headline("inversions")).toBe("Inversions: 1");
    expect(findingHeadline(finding("envelope"), statistics, "metric").location).toEqual(turn);
    for (const line of findings.map((candidate) => headline(candidate.key))) {
      expect(line).not.toMatch(/weighted seconds|g-s|lateral|magnitude/);
      expect(line.length).toBeLessThan(130);
    }
  });

  it("formats headlines for the other terms in the selected units", () => {
    const single = (contribution: ScoreContributionReport, units: "metric" | "imperial" = "metric"): string =>
      formatFindingHeadline(findingHeadline(groupScoreFindings({ excitement: [contribution], intensity: [], nausea: [] })[0], statistics, units));
    expect(single({ term: "topSpeed", value: 0.7, measure: 18.4, unit: "m/s" }, "imperial")).toBe("Top speed: 68 mph");
    expect(single({ term: "biggestDrop", value: 0.6, measure: 48, unit: "m" }, "imperial")).toBe("Biggest drop: 157 ft");
    expect(single({ term: "deadTime", value: -0.5, measure: 37.1, unit: "s", location: { ...loop, pieceName: "One-tile straight" } }))
      .toBe("Uneventful stretches, longest at One-tile straight: 37.1 s more than allowed");
    expect(single({ term: "airtime", value: 0.6, measure: 1.2, unit: "s", forceDetails: [
      { axis: "airtimeGood", measuredValue: 0.8, limit: 0.2, unit: "g", durationSeconds: 3, weight: 0.55, location: loop },
    ] })).toBe("Airtime at Vertical loop: down to −0.8 g, 3.0 s below −0.2 g");
    expect(single({ term: "envelope", value: -0.3, measure: 0.2, unit: "g-s", forceDetail: { axis: "onset", measuredValue: 18, limit: 15, unit: "g/s", durationSeconds: 0.1, location: loop } }))
      .toBe("Past the ASTM comfort envelope at Vertical loop: vertical force rising at 18.0 g per second for 0.1 s (limit 15.0 g per second)");
    expect(single({ term: "envelope", value: 0.4, measure: 0.2, unit: "g-s", forceDetail: { axis: "verticalLateral", measuredValue: 1.2, limit: 1, unit: "ratio", durationSeconds: 0.8, location: turn } }))
      .toBe("Past the ASTM comfort envelope at Begin large left turn: vertical and sideways force together at 1.2× the limit for 0.8 s");
    expect(single({ term: "launch", value: 0.7, measure: 1.5, unit: "g", forceDetails: [{ axis: "launchForward", measuredValue: 1.5, limit: 0, unit: "g", location: loop }] }))
      .toBe("Launch at Vertical loop: 1.5 g forward");
    for (const term of Object.keys(findingTitles)) {
      expect(() => single({ term, value: 0.3, measure: 1, unit: "" })).not.toThrow();
    }
  });

  it("keeps every number behind a finding for the expanded view", () => {
    expect(findingDetails(finding("excessiveForces"), "metric")).toEqual([
      "Vertical force 5.4 g for 0.2 s (limit 4.5 g) (car 6) at End gentle descent, 405.9 m.",
      "Sideways force 4.8 g for 3.5 s (limit 1.5 g) (car 1) at Begin large left turn, 582.1 m; 3.3 s of it far past the limit, which counts double.",
      "The limits are the comfortable forces for this coaster model.",
      "Score input: 7.1 s over the limits.",
      "Car numbers count from the front.",
    ]);
    expect(findingDetails(finding("positiveForces"), "imperial")).toEqual([
      "Good push into the seat (above 2.0 g): 2.3 s (car 6), peak 3.6 g at Vertical loop, 1,463 ft; counts 55%.",
      "Great push into the seat (above 3.6 g): 1.6 s (car 1), peak 4.5 g at Vertical loop, 1,463 ft.",
      "Score input: 3.1 s weighted (time in milder bands counts less).",
      "Car numbers count from the front.",
    ]);
    expect(findingDetails(finding("inversions"), "metric")).toEqual([
      "Excitement input: 0.9 (repeated elements and inversions without heartlining count less).",
      "Intensity input: 1.0 (repeated elements and inversions without heartlining count less).",
    ]);
    expect(findingDetails(finding("envelope"), "metric")[1]).toBe("Score input: 0.4 g × seconds past the envelope (how far times how long).");
  });
});
