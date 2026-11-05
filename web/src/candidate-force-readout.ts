import { rateForce, rateVertical } from "./force-overlay";
import { formatSpeed, type Units } from "./units";

import type { ConstructionCandidateForcePreviewWire } from "./generated/ConstructionCandidateForcePreviewWire";

export type CandidateForcePreview = Omit<ConstructionCandidateForcePreviewWire, "status"> & {
  readonly status: "reachedEnd" | "stalled" | "timedOut";
};

function formatForce(force: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 1 }).format(force).replace("-", "−");
}

function verticalRating(force: number, limits: CandidateForcePreview["limits"]): string {
  const { axis, rating } = rateVertical(force, limits);
  return axis === "airtime" ? `${rating} airtime` : rating;
}

export interface HillForcePreview {
  readonly entrySpeed: number | null;
  readonly source: string;
  readonly ratedSpeed: CandidateForcePreview["ratedSpeed"];
  readonly minimumVerticalG: number | null;
  readonly maximumVerticalG: number | null;
  readonly airtimeSeconds: number | null;
}

function speedReadout(entrySpeed: number, ratedSpeed: CandidateForcePreview["ratedSpeed"], units: Units): string {
  const range = ratedSpeed
    ? ` (${entrySpeed >= ratedSpeed.minimumEntry && entrySpeed <= ratedSpeed.maximumEntry ? "within" : "outside"} rated range ${formatSpeed(ratedSpeed.minimumEntry, units).replace(/ (mph|km\/h)$/, "")}–${formatSpeed(ratedSpeed.maximumEntry, units)})`
    : "";
  return `Estimated entry speed: ${formatSpeed(entrySpeed, units)}${range}`;
}

function hillForceReadout(preview: HillForcePreview, units: Units): string[] {
  if (preview.source === "timedOut") return ["The estimate timed out before this hill; no force estimate is available."];
  if (preview.entrySpeed === null || preview.minimumVerticalG === null || preview.maximumVerticalG === null || preview.airtimeSeconds === null) return ["Rollback expected; no hill force estimate is available."];
  const { minimumVerticalG, maximumVerticalG, airtimeSeconds } = preview;
  return [
    `Hill profile estimate (entry speed: ${preview.source === "simulated" ? "simulated" : "energy estimate"})`,
    speedReadout(preview.entrySpeed, preview.ratedSpeed, units),
    `Estimated minimum vertical force: ${formatForce(minimumVerticalG)} g; maximum vertical force: ${formatForce(maximumVerticalG)} g`,
    `Estimated airtime: ${formatForce(airtimeSeconds)} s`,
  ];
}

export function candidateForceReadout(preview: CandidateForcePreview | null, units: Units, hill: HillForcePreview | null = null): string[] {
  const lines = preview ? simulatedForceReadout(preview, units) : [];
  return hill ? [...lines, ...hillForceReadout(hill, units)] : lines;
}

function simulatedForceReadout(preview: CandidateForcePreview, units: Units): string[] {
  if (preview.entrySpeed === null) {
    return ["The train does not reach this piece; no force estimate is available."];
  }
  if (preview.peaks === null) return ["The train reaches this piece, but no force samples are available."];
  const lines = [
    "Front-car force estimates (sampled along the track)",
    speedReadout(preview.entrySpeed, preview.ratedSpeed, units),
    `Estimated minimum vertical force: ${formatForce(preview.peaks.minimumVerticalG)} g (${verticalRating(preview.peaks.minimumVerticalG, preview.limits)})`,
    `Estimated maximum vertical force: ${formatForce(preview.peaks.maximumVerticalG)} g (${verticalRating(preview.peaks.maximumVerticalG, preview.limits)})`,
    `Estimated ${preview.outerBank ? "outer-bank lateral" : "lateral"} force: ${formatForce(preview.peaks.maximumAbsoluteLateralG)} g (${rateForce("lateral", preview.peaks.maximumAbsoluteLateralG, preview.limits)}); longitudinal force: ${formatForce(preview.peaks.maximumAbsoluteLongitudinalG)} g (${rateForce("longitudinal", preview.peaks.maximumAbsoluteLongitudinalG, preview.limits)})`,
  ];
  if (preview.status === "stalled") lines.unshift("The train stalls in this piece; force estimates are partial.");
  if (preview.status === "timedOut") lines.unshift("The estimate timed out in this piece; force estimates are partial.");
  return lines;
}
