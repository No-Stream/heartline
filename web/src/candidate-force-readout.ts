import { evaluatedForceRating, rateForce, rateVertical, type ForceLimits, type ForceRating } from "./force-overlay";
import { formatLength, formatSpeed, type Units } from "./units";

import type { ConstructionCandidateForcePreviewWire } from "./generated/ConstructionCandidateForcePreviewWire";
import type { ConstructionHillEstimateWire } from "./generated/ConstructionHillEstimateWire";
import type { ConstructionShapedElementWire } from "./generated/ConstructionShapedElementWire";

export type CandidateForcePreview = Omit<ConstructionCandidateForcePreviewWire, "status"> & {
  readonly status: "reachedEnd" | "stalled" | "timedOut";
};

function formatForce(force: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 1 }).format(force).replace("-", "−");
}

function formatCalibratedForce(force: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(force).replace("-", "−");
}

function shapeRefusalMessage(shape: ConstructionShapedElementWire, units: Units): string[] {
  if (!shape.refusal) return [];
  switch (shape.refusal.kind) {
    case "stall":
      return [`Would stall at ${formatSpeed(shape.refusal.entrySpeedMps, units, 0)}; tallest feasible hill is ${formatLength(shape.refusal.heightM, units)}.`];
    case "infeasibleTarget":
      return [`Target ${formatCalibratedForce(shape.refusal.targetG)} g exceeds available headroom of ${formatCalibratedForce(shape.refusal.availableHeadroomG)} g.`];
    case "belowMinimumSpeed":
      return [`Entry speed is below the shaped-element minimum of ${formatSpeed(shape.refusal.minimumSpeedMps, units, 1)}.`];
    case "unsupportedScope":
      return ["Shaped elements require a hyper coaster with 4–8 cars and entry speed up to 36 m/s."];
    case "previewTimedOut":
      return ["The train preview timed out before this shape could be calibrated."];
    case "invalidContext":
      return ["The train cannot reach this element's entry connector."];
    case "unsupportedConnector":
      return ["Shaped elements need a level, straight entry connector."];
    case "unsupportedSizeOverride":
      return ["This fixed size override is not supported for shaped elements."];
    case "invalidGeometry":
      return ["The shaped element geometry could not be integrated at this entry speed."];
    case "geometryDigestMismatch":
      return ["Saved shaped geometry does not match its recipe."];
  }
}

export function shapedElementReadout(shape: ConstructionShapedElementWire, units: Units): string[] {
  const refusal = shapeRefusalMessage(shape, units);
  if (refusal.length > 0) return refusal;
  if (!shape.spec) return ["Calibrating this shaped element…"];

  const { spec } = shape;
  const trainLine = `Design speed: ${formatSpeed(spec.designSpeedMps, units, 0)} · ${spec.cars}-car train`;
  if (spec.kind === "hill") {
    if (spec.targetG === null || spec.deliveredG === null || spec.heightM === null || spec.spanM === null) {
      throw new Error("Shaped hill calibration is missing target, delivered force, or dimensions");
    }
    return [
      trainLine,
      `Airtime target: ${formatCalibratedForce(spec.targetG)} g · calibrated: ${formatCalibratedForce(spec.deliveredG)} g`,
      `Resolved size: ${formatLength(spec.heightM, units)} high × ${formatLength(spec.spanM, units)} span`,
    ];
  }
  if (spec.radiusM === null || spec.bankDegrees === null || spec.lateralG === null) {
    throw new Error("Shaped turn calibration is missing radius, bank, or lateral force");
  }
  return [
    trainLine,
    `Resolved size: ${formatLength(spec.radiusM, units)} radius · ${formatCalibratedForce(spec.bankDegrees)}° bank`,
    `Lateral force at design speed: ${formatCalibratedForce(spec.lateralG)} g`,
  ];
}

function verticalRating(force: number, limits: CandidateForcePreview["limits"], exceeded: boolean): string {
  const { axis } = rateVertical(force, limits);
  const rating = evaluatedForceRating(axis, force, limits, exceeded);
  return axis === "airtime" ? `${rating} airtime` : rating;
}

function speedReadout(entrySpeed: number, ratedSpeed: CandidateForcePreview["ratedSpeed"], units: Units): string {
  const range = ratedSpeed
    ? ` (${entrySpeed >= ratedSpeed.minimumEntry && entrySpeed <= ratedSpeed.maximumEntry ? "within" : "outside"} rated range ${formatSpeed(ratedSpeed.minimumEntry, units).replace(/ (mph|km\/h)$/, "")}–${formatSpeed(ratedSpeed.maximumEntry, units)})`
    : "";
  return `Estimated entry speed: ${formatSpeed(entrySpeed, units)}${range}`;
}

export type HillForcePreview = ConstructionHillEstimateWire;
/** How a readout value reads against the rating bands: weak, fine, great, past the limit, or dangerous. */
export type HillReadoutTone = "weak" | "fine" | "great" | "past" | "dangerous";
export interface HillReadoutRow {
  readonly label: string;
  readonly value: string;
  readonly note: string;
  readonly tone: HillReadoutTone | null;
}
export interface HillReadout {
  readonly rows: readonly HillReadoutRow[];
  /** Why there is no force estimate, or empty when there is one. */
  readonly message: string;
}

const ratingTones: Readonly<Record<ForceRating, HillReadoutTone>> = {
  brief: "great", weak: "weak", good: "fine", fine: "fine", great: "great", strong: "great", "too much": "past", dangerous: "dangerous",
};

function verticalRow(label: string, force: number, limits: ForceLimits): HillReadoutRow {
  const { axis, rating } = rateVertical(force, limits);
  return { label, value: `${formatForce(force)} g`, note: axis === "airtime" ? `${rating} airtime` : rating, tone: ratingTones[rating] };
}

/** The airtime hill inspector's prediction: entry speed against the hill's rated window, then the forces and airtime it gives at that speed. */
export function hillReadout(preview: HillForcePreview, units: Units): HillReadout {
  const speed = (metresPerSecond: number): string => formatSpeed(metresPerSecond, units, 0);
  const { ratedSpeed, entrySpeed } = preview;
  const ratedRow = (entry: number | null): HillReadoutRow[] => {
    if (!ratedSpeed) return [];
    const note = entry === null ? "" : entry < ratedSpeed.minimumEntry ? "too slow" : entry > ratedSpeed.maximumEntry ? "too fast" : "in range";
    return [{ label: "Rated for", value: `${speed(ratedSpeed.minimumEntry).replace(/ (mph|km\/h)$/, "")}–${speed(ratedSpeed.maximumEntry)}`, note, tone: note === "" ? null : note === "in range" ? "fine" : "past" }];
  };
  if (preview.source === "timedOut") return { rows: ratedRow(null), message: "The speed estimate timed out before this hill." };
  if (entrySpeed === null) return { rows: ratedRow(null), message: "The train would roll back before reaching this hill." };
  if (preview.source === "stalled" || entrySpeed === 0) return { rows: ratedRow(null), message: "The train stalls before reaching this hill." };
  const entryRows = [{ label: "Entry speed", value: speed(entrySpeed), note: "", tone: null }, ...ratedRow(entrySpeed)];
  const { minimumVerticalG, maximumVerticalG, airtimeSeconds } = preview;
  if (minimumVerticalG === null || maximumVerticalG === null || airtimeSeconds === null) {
    return { rows: entryRows, message: "At this speed the train would not make it over the crest." };
  }
  return {
    rows: [
      ...entryRows,
      verticalRow("Lowest", minimumVerticalG, preview.limits),
      verticalRow("Highest", maximumVerticalG, preview.limits),
      ...(preview.maximumAbsoluteLateralG === null ? [] : [{
        label: "Lateral", value: `${formatForce(preview.maximumAbsoluteLateralG)} g`,
        note: rateForce("lateral", preview.maximumAbsoluteLateralG, preview.limits),
        tone: ratingTones[rateForce("lateral", preview.maximumAbsoluteLateralG, preview.limits)],
      }]),
      { label: "Airtime", value: `${formatForce(airtimeSeconds)} s`, note: "", tone: null },
    ],
    message: "",
  };
}

export function candidateForceReadout(preview: CandidateForcePreview, units: Units): string[] {
  if (preview.entrySpeed === null) {
    return ["The train does not reach this piece; no force estimate is available."];
  }
  if (preview.peaks === null) return ["The train reaches this piece, but no force samples are available."];
  const lines = [
    "Worst-car force estimates (sampled along the track)",
    "Placement estimate is provisional until the following piece exists.",
    speedReadout(preview.entrySpeed, preview.ratedSpeed, units),
    `Estimated minimum vertical force: ${formatForce(preview.peaks.minimumVerticalG)} g (${verticalRating(preview.peaks.minimumVerticalG, preview.limits, preview.peaks.minimumVerticalLimitExceeded)})`,
    `Estimated maximum vertical force: ${formatForce(preview.peaks.maximumVerticalG)} g (${verticalRating(preview.peaks.maximumVerticalG, preview.limits, preview.peaks.maximumVerticalLimitExceeded)})`,
    `Estimated ${preview.outerBank ? "outer-bank lateral" : "lateral"} force: ${formatForce(preview.peaks.maximumAbsoluteLateralG)} g (${evaluatedForceRating("lateral", preview.peaks.maximumAbsoluteLateralG, preview.limits, preview.peaks.lateralLimitExceeded)}); longitudinal force: ${formatForce(preview.peaks.maximumAbsoluteLongitudinalG)} g (${evaluatedForceRating("longitudinal", preview.peaks.maximumAbsoluteLongitudinalG, preview.limits, preview.peaks.longitudinalLimitExceeded)})`,
  ];
  if (preview.status === "stalled") lines.unshift("The train stalls in this piece; force estimates are partial.");
  if (preview.status === "timedOut") lines.unshift("The estimate timed out in this piece; force estimates are partial.");
  return lines;
}
