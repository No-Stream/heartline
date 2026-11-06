import { formatLength, formatSpeed, type Units } from "./units";
import type { RideStatisticsReport } from "./generated/RideStatisticsReport";
import type { ScoreBreakdownReport } from "./generated/ScoreBreakdownReport";
import type { ScoreContributionReport } from "./generated/ScoreContributionReport";
import type { ScoreForceDetailReport } from "./generated/ScoreForceDetailReport";
import type { ScoreLocationReport } from "./generated/ScoreLocationReport";

/**
 * Turns the per-score contribution lists into findings a player can read at a glance: each scoring term once, with its
 * effect on every score it touches, a short headline, and the full numbers kept for an expanded view.
 */
export type ScoreName = keyof ScoreBreakdownReport;
export const scoreNames: readonly ScoreName[] = ["excitement", "intensity", "nausea"];
export const scoreLabels: Readonly<Record<ScoreName, string>> = { excitement: "Excitement", intensity: "Intensity", nausea: "Nausea" };

export interface ScoreFinding {
  readonly key: string;
  readonly effects: Partial<Record<ScoreName, ScoreContributionReport>>;
  /** Sum of the absolute effects on all three scores; findings are ranked by it. */
  readonly size: number;
}

export interface FindingHeadline {
  readonly title: string;
  readonly location?: ScoreLocationReport;
  readonly measurement?: string;
}

/** Contributions smaller than this print as ±0.0, so they count as having no effect. */
const visibleEffect = 0.05;
export const mainFindingCount = 5;

// Excitement's positive forces and nausea's strong positive forces read the same weighted seconds and force bands.
const findingKeyOfTerm: Readonly<Record<string, string>> = { greyOut: "positiveForces" };

export const findingTitles: Readonly<Record<string, string>> = {
  excitementFloor: "Excitement cannot drop below zero",
  airtime: "Airtime",
  positiveForces: "Pushed into the seat",
  laterals: "Strong sideways force",
  longitudinalForces: "Strong forward or backward force",
  excessiveForces: "Over the train's force limit",
  airtimeMoments: "Airtime moments",
  inversions: "Inversions",
  topSpeed: "Top speed",
  biggestDrop: "Biggest drop",
  pace: "Pace",
  launch: "Launch",
  variety: "Variety of elements",
  whip: "Whip",
  rollRate: "Fastest roll",
  fastRolls: "Time in fast rolls",
  rideTime: "Ride length",
  shortRide: "Short ride",
  proximity: "Close passes",
  theming: "Theming",
  deadTime: "Uneventful stretches",
  envelope: "Past the ASTM comfort envelope",
  sickening: "High nausea puts riders off",
  peakForce: "Strongest force",
  sustainedForce: "Time under strong forces",
  reversals: "Reversals",
  greyOut: "Pushed into the seat",
  timeInverted: "Time upside down",
};

const number = (value: number): string => value.toFixed(1).replace("-", "−");
const signed = (value: number): string => value >= 0 ? `+${number(value)}` : number(value);
const g = (value: number): string => `${number(value)} g`;
const seconds = (value: number): string => `${number(value)} s`;
const percent = (fraction: number): string => `${Math.round(fraction * 100)}%`;
const place = (location: ScoreLocationReport, units: Units): string => `${location.pieceName}, ${formatLength(location.distance, units)}`;
const car = (detail: ScoreForceDetailReport): string => detail.carIndex === undefined ? "" : ` (car ${detail.carIndex + 1})`;

export function groupScoreFindings(breakdown: ScoreBreakdownReport): ScoreFinding[] {
  const findings = new Map<string, { key: string; effects: Partial<Record<ScoreName, ScoreContributionReport>> }>();
  for (const score of scoreNames) {
    for (const contribution of breakdown[score]) {
      if (findingTitles[contribution.term] === undefined) throw new Error(`Unknown coaster score term: ${contribution.term}`);
      const key = findingKeyOfTerm[contribution.term] ?? contribution.term;
      const finding = findings.get(key) ?? { key, effects: {} };
      if (finding.effects[score]) throw new Error(`Score term ${key} appears twice in the ${score} breakdown`);
      finding.effects[score] = contribution;
      findings.set(key, finding);
    }
  }
  return [...findings.values()]
    .map((finding) => ({ ...finding, size: scoreNames.reduce((total, score) => total + Math.abs(finding.effects[score]?.value ?? 0), 0) }))
    .sort((left, right) => right.size - left.size);
}

export function hasVisibleEffect(finding: ScoreFinding): boolean {
  return scoreNames.some((score) => Math.abs(finding.effects[score]?.value ?? 0) >= visibleEffect);
}

/** The biggest findings, the smaller ones folded behind "More detail", and the terms that did not move any score. */
export function splitScoreFindings(findings: readonly ScoreFinding[]): { main: ScoreFinding[]; more: ScoreFinding[]; unchanged: ScoreFinding[] } {
  const visible = findings.filter(hasVisibleEffect);
  return {
    main: visible.slice(0, mainFindingCount),
    more: visible.slice(mainFindingCount),
    // The excitement floor only matters when it lifts a negative total; listing it as "no effect" would read as a ride trait.
    unchanged: findings.filter((finding) => !hasVisibleEffect(finding) && finding.key !== "excitementFloor"),
  };
}

/** Each visible effect as a score name and signed value, in excitement, intensity, nausea order. */
export function findingEffects(finding: ScoreFinding): { score: ScoreName; value: number }[] {
  return scoreNames.flatMap((score) => {
    const value = finding.effects[score]?.value;
    return value !== undefined && Math.abs(value) >= visibleEffect ? [{ score, value }] : [];
  });
}

export function formatFindingEffects(finding: ScoreFinding): string {
  return findingEffects(finding).map(({ score, value }) => `${scoreLabels[score]} ${signed(value)}`).join(" · ");
}

export function formatFindingHeadline(headline: FindingHeadline): string {
  const location = headline.location ? ` at ${headline.location.pieceName}` : "";
  return `${headline.title}${location}${headline.measurement ? `: ${headline.measurement}` : ""}`;
}

function primaryContribution(finding: ScoreFinding): ScoreContributionReport {
  const contributions = scoreNames.flatMap((score) => finding.effects[score] ?? []);
  return contributions.reduce((largest, contribution) => Math.abs(contribution.value) > Math.abs(largest.value) ? contribution : largest);
}

const forceAxes: Readonly<Record<string, string>> = {
  vertical: "vertical force", negativeVertical: "negative vertical force", lateral: "sideways force", longitudinal: "forward or backward force",
  verticalLateral: "vertical and sideways force together", verticalLongitudinal: "vertical and forward force together",
  lateralLongitudinal: "sideways and forward force together", onset: "vertical force rising",
  peakVertical: "vertical", peakNegativeVertical: "negative vertical", peakLateral: "sideways", peakForward: "forward", peakBraking: "braking",
};

function forceAxis(detail: ScoreForceDetailReport): string {
  const axis = forceAxes[detail.axis];
  if (axis === undefined) throw new Error(`Unknown scoring force axis: ${detail.axis}`);
  return axis;
}

/** Negative vertical force is reported as a magnitude; players read it as a negative number. */
const signedForce = (detail: ScoreForceDetailReport, value: number): string =>
  detail.axis === "negativeVertical" || detail.axis === "peakNegativeVertical" ? g(-value) : g(value);

function requireDuration(detail: ScoreForceDetailReport): number {
  if (detail.durationSeconds === undefined) throw new Error(`Missing duration for scoring force axis: ${detail.axis}`);
  return detail.durationSeconds;
}

function limitMeasurement(detail: ScoreForceDetailReport): string {
  const duration = seconds(requireDuration(detail));
  if (detail.unit === "ratio") return `${forceAxis(detail)} at ${number(detail.measuredValue / detail.limit)}× the limit for ${duration}`;
  if (detail.unit === "g/s") return `${forceAxis(detail)} at ${number(detail.measuredValue)} g per second for ${duration} (limit ${number(detail.limit)} g per second)`;
  return `${forceAxis(detail)} ${signedForce(detail, detail.measuredValue)} for ${duration} (limit ${signedForce(detail, detail.limit)})`;
}

const forceBands: Readonly<Record<string, { name: string; below?: boolean }>> = {
  airtimeGood: { name: "good airtime", below: true }, airtimeGreat: { name: "great airtime", below: true },
  verticalGood: { name: "good push into the seat" }, verticalGreat: { name: "great push into the seat" },
  lateralStrong: { name: "strong sideways force" }, longitudinalStrong: { name: "strong forward or backward force" },
};

function forceBand(detail: ScoreForceDetailReport): { name: string; below?: boolean } {
  const band = forceBands[detail.axis];
  if (band === undefined) throw new Error(`Unknown scoring force band: ${detail.axis}`);
  return band;
}

function bandMeasurement(detail: ScoreForceDetailReport): string {
  const band = forceBand(detail);
  const duration = seconds(requireDuration(detail));
  return band.below
    ? `down to ${g(-detail.measuredValue)}, ${duration} below ${g(-detail.limit)}`
    : `up to ${g(detail.measuredValue)}, ${duration} above ${g(detail.limit)}`;
}

/** The detail with the strongest band (the bands are listed mildest first) stands for the finding in its headline. */
function strongestBand(details: readonly ScoreForceDetailReport[]): ScoreForceDetailReport | undefined {
  return details.at(-1);
}

export function findingHeadline(finding: ScoreFinding, statistics: RideStatisticsReport, units: Units): FindingHeadline {
  const title = findingTitles[finding.key];
  const contribution = primaryContribution(finding);
  const details = contribution.forceDetails ?? [];
  const measure = contribution.measure;
  switch (finding.key) {
    case "envelope": {
      const detail = contribution.forceDetail;
      return detail ? { title, location: detail.location, measurement: limitMeasurement(detail) } : { title };
    }
    case "excessiveForces": {
      if (!details.length) return { title, measurement: `${seconds(measure)} in total` };
      const lead = details.reduce((longest, detail) => requireDuration(detail) > requireDuration(longest) ? detail : longest);
      const others = details.length - 1;
      return { title, location: lead.location, measurement: `${limitMeasurement(lead)}${others ? `, plus ${others} other ${others === 1 ? "force" : "forces"}` : ""}` };
    }
    case "airtime": case "positiveForces": case "laterals": case "longitudinalForces": {
      const detail = strongestBand(details);
      if (detail) return { title, location: detail.location, measurement: bandMeasurement(detail) };
      return { title, measurement: finding.key === "airtime" || finding.key === "positiveForces" ? `${seconds(measure)}, weighted` : seconds(measure) };
    }
    case "sustainedForce": return { title, measurement: `${seconds(measure)}, weighted` };
    case "peakForce": {
      const detail = details[0];
      return detail
        ? { title, location: detail.location, measurement: `${forceAxis(detail)} ${signedForce(detail, detail.measuredValue)} (scoring reference ${signedForce(detail, detail.limit)})` }
        : { title, measurement: `${number(measure)}× the scoring reference` };
    }
    case "launch": {
      const detail = details[0];
      return detail ? { title, location: detail.location, measurement: `${g(detail.measuredValue)} forward` } : { title, measurement: "none" };
    }
    case "deadTime": return { title: contribution.location ? "Uneventful stretches, longest" : title, location: contribution.location, measurement: `${seconds(measure)} more than allowed` };
    case "topSpeed": return { title, measurement: formatSpeed(statistics.maxSpeed, units) };
    case "biggestDrop": return { title, measurement: formatLength(measure, units) };
    case "inversions": return { title, measurement: `${statistics.inversions}` };
    case "pace": return { title, measurement: `${formatSpeed(measure, units)} above the threshold` };
    case "rollRate": return { title, measurement: `${number(measure)}° per second above the threshold` };
    case "rideTime": case "shortRide": return { title, measurement: seconds(measure) };
    case "fastRolls": case "timeInverted": return { title, measurement: seconds(measure) };
    case "variety": return { title, measurement: `${Math.round(measure)} extra element types` };
    case "whip": return { title, measurement: `${Math.round(measure)} quick swings in vertical force` };
    case "airtimeMoments": case "reversals": return { title, measurement: `${Math.round(measure)}` };
    case "proximity": return { title, measurement: `closeness ${number(measure)} in total` };
    case "theming": return { title, measurement: percent(measure) };
    case "sickening": return { title, measurement: `nausea ${number(measure)}` };
    case "excitementFloor": return { title };
    default: throw new Error(`No headline for coaster score term: ${finding.key}`);
  }
}

function scoreInputs(finding: ScoreFinding, describe: (measure: number) => string): string[] {
  const inputs = scoreNames.flatMap((score) => finding.effects[score] ? [{ score, measure: finding.effects[score].measure }] : []);
  if (inputs.every((input) => Math.abs(input.measure - inputs[0].measure) < 0.05)) return [`Score input: ${describe(inputs[0].measure)}.`];
  return inputs.map((input) => `${scoreLabels[input.score]} input: ${describe(input.measure)}.`);
}

function forceDetailLine(detail: ScoreForceDetailReport, units: Units): string {
  if (detail.axis in forceBands) {
    const band = forceBand(detail);
    const weight = detail.weight !== undefined && detail.weight !== 1 ? `; counts ${percent(detail.weight)}` : "";
    const threshold = band.below ? `below ${g(-detail.limit)}` : `above ${g(detail.limit)}`;
    const peak = band.below ? `down to ${g(-detail.measuredValue)}` : `peak ${g(detail.measuredValue)}`;
    return `${band.name[0].toUpperCase()}${band.name.slice(1)} (${threshold}): ${seconds(requireDuration(detail))}${car(detail)}, ${peak} at ${place(detail.location, units)}${weight}.`;
  }
  if (detail.axis === "launchForward") return `Strongest launch: ${g(detail.measuredValue)} forward${car(detail)} at ${place(detail.location, units)}.`;
  if (detail.durationSeconds === undefined) {
    return `Strongest: ${forceAxis(detail)} ${signedForce(detail, detail.measuredValue)}${car(detail)} at ${place(detail.location, units)}; scoring reference ${signedForce(detail, detail.limit)}.`;
  }
  const measurement = limitMeasurement(detail);
  const dangerous = detail.dangerousSeconds ? `; ${seconds(detail.dangerousSeconds)} of it far past the limit, which counts double` : "";
  return `${measurement[0].toUpperCase()}${measurement.slice(1)}${car(detail)} at ${place(detail.location, units)}${dangerous}.`;
}

const weightedSeconds = (measure: number): string => `${seconds(measure)} weighted (time in milder bands counts less)`;

/** Every number behind a finding, one sentence each, for the expanded view. */
export function findingDetails(finding: ScoreFinding, units: Units): string[] {
  const contribution = primaryContribution(finding);
  const forceDetails = [...(contribution.forceDetail ? [contribution.forceDetail] : []), ...(contribution.forceDetails ?? [])];
  const forces = forceDetails.map((detail) => forceDetailLine(detail, units));
  const inputs = ((): string[] => {
    switch (finding.key) {
      case "envelope": return scoreInputs(finding, (measure) => `${number(measure)} g × seconds past the envelope (how far times how long)`);
      case "excessiveForces": return [
        "The limits are the comfortable forces for this coaster model.",
        ...scoreInputs(finding, (measure) => `${seconds(measure)} over the limits`),
      ];
      case "airtime": case "positiveForces": case "sustainedForce": return scoreInputs(finding, weightedSeconds);
      case "laterals": case "longitudinalForces": case "fastRolls": case "timeInverted": case "rideTime": case "shortRide": return scoreInputs(finding, seconds);
      case "peakForce": return scoreInputs(finding, (measure) => `${number(measure)}× the scoring reference`);
      case "launch": return scoreInputs(finding, (measure) => g(measure));
      case "deadTime": return [
        ...scoreInputs(finding, (measure) => `${seconds(measure)} of uneventful riding beyond the allowance`),
        ...(contribution.location ? [`Longest uneventful stretch: ${place(contribution.location, units)}.`] : []),
      ];
      case "topSpeed": case "pace": return scoreInputs(finding, (measure) => `${formatSpeed(measure, units)} above this score's threshold`);
      case "biggestDrop": return scoreInputs(finding, (measure) => formatLength(measure, units));
      case "inversions": return scoreInputs(finding, (measure) => `${number(measure)} (repeated elements and inversions without heartlining count less)`);
      case "rollRate": return scoreInputs(finding, (measure) => `${number(measure)}° per second above the threshold`);
      case "proximity": return scoreInputs(finding, (measure) => `total closeness ${number(measure)} from near misses, tunnels and other close passes`);
      case "theming": return scoreInputs(finding, percent);
      case "sickening": return ["Excitement drops once nausea passes a threshold."];
      case "excitementFloor": return ["The other excitement effects add up to less than zero, so this brings excitement back to zero."];
      default: return scoreInputs(finding, number);
    }
  })();
  const cars = forceDetails.some((detail) => detail.carIndex !== undefined) ? ["Car numbers count from the front."] : [];
  return [...forces, ...inputs, ...cars];
}
