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

export function candidateForceReadout(preview: CandidateForcePreview, units: Units): string[] {
  if (preview.entrySpeed === null) {
    return ["The train does not reach this piece; no force estimate is available."];
  }
  if (preview.peaks === null) return ["The train reaches this piece, but no force samples are available."];
  const { ratedSpeed } = preview;
  const range = ratedSpeed
    ? ` (${preview.entrySpeed >= ratedSpeed.minimumEntry && preview.entrySpeed <= ratedSpeed.maximumEntry ? "within" : "outside"} rated range ${formatSpeed(ratedSpeed.minimumEntry, units).replace(/ (mph|km\/h)$/, "")}–${formatSpeed(ratedSpeed.maximumEntry, units)})`
    : "";
  const lines = [
    "Front-car force estimates (sampled along the track)",
    `Estimated entry speed: ${formatSpeed(preview.entrySpeed, units)}${range}`,
    `Estimated minimum vertical force: ${formatForce(preview.peaks.minimumVerticalG)} g (${verticalRating(preview.peaks.minimumVerticalG, preview.limits)})`,
    `Estimated maximum vertical force: ${formatForce(preview.peaks.maximumVerticalG)} g (${verticalRating(preview.peaks.maximumVerticalG, preview.limits)})`,
    `Estimated ${preview.outerBank ? "outer-bank lateral" : "lateral"} force: ${formatForce(preview.peaks.maximumAbsoluteLateralG)} g (${rateForce("lateral", preview.peaks.maximumAbsoluteLateralG, preview.limits)}); longitudinal force: ${formatForce(preview.peaks.maximumAbsoluteLongitudinalG)} g (${rateForce("longitudinal", preview.peaks.maximumAbsoluteLongitudinalG, preview.limits)})`,
  ];
  if (preview.status === "stalled") lines.unshift("The train stalls in this piece; force estimates are partial.");
  if (preview.status === "timedOut") lines.unshift("The estimate timed out in this piece; force estimates are partial.");
  return lines;
}
