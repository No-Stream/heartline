import type { ConstructionForcePreview } from "./construction-protocol";
import type { ConstructionSpeedMatchWire } from "./generated/ConstructionSpeedMatchWire";

export type ConstructionSpeedMatch = ConstructionSpeedMatchWire;

export function speedMatchedSelection(piece: string, match: ConstructionSpeedMatch | null, exact: boolean): string {
  return exact ? piece : match?.recommendedId ?? piece;
}

export function speedMatchMessage(match: ConstructionSpeedMatch, selected: string | null): string {
  if (match.entrySpeed === null) return "The train does not reach this piece, so a size cannot be recommended.";
  const recommendation = match.variants.find(({ id }) => id === match.recommendedId);
  if (match.fits === false) return `No available size fits the estimated entry speed.${recommendation ? ` ${recommendation.label} has the nearest rated range.` : ""}`;
  if (!recommendation) throw new Error("Missing recommended construction variant");
  const reason = match.variants.length > 1
    ? "its rated range contains the entry speed with the greatest margin to either edge"
    : "its rated range contains the entry speed";
  const selectedVariant = match.variants.find(({ id }) => id === selected);
  const override = selectedVariant && selectedVariant.id !== recommendation.id ? ` You selected ${selectedVariant.label}.` : "";
  return `${recommendation.label} is recommended: ${reason}.${override}`;
}

export function constructionEndSpeed(preview: ConstructionForcePreview | null): number | undefined {
  return preview?.status === "reachedEnd" ? preview.samples.at(-1)?.speed : undefined;
}
