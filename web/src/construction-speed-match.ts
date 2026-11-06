import type { ConstructionForcePreview, ConstructionSpeedTurn } from "./construction-protocol";
import type { ConstructionSpeedMatchWire } from "./generated/ConstructionSpeedMatchWire";
import { formatSpeed, type Units } from "./units";

export type ConstructionSpeedMatch = ConstructionSpeedMatchWire;

const unreachableMessage = "The train does not reach this piece, so a size cannot be recommended.";

export function speedMatchedSelection(piece: string, match: ConstructionSpeedMatch | null, exact: boolean): string {
  return exact ? piece : match?.recommendedId ?? piece;
}

export function speedMatchMessage(match: ConstructionSpeedMatch, selected: string | null): string {
  if (match.entrySpeed === null) return unreachableMessage;
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


export function speedTurnSizeLabel(designSpeed: number, units: Units): string {
  return `Banked for ${formatSpeed(designSpeed, units, 0)}`;
}

/** The catalog names a speed-matched turn by its design speed in m/s; the player reads it in their own units. */
export function speedTurnName(label: string, designSpeed: number, units: Units): string {
  const catalogSpeed = `banked for ${designSpeed} m/s`;
  if (!label.endsWith(catalogSpeed)) throw new Error(`Speed-matched turn name "${label}" does not end with "${catalogSpeed}"`);
  return `${label.slice(0, -catalogSpeed.length)}${speedTurnSizeLabel(designSpeed, units).toLowerCase()}`;
}

export function speedTurnMessage(match: ConstructionSpeedMatch, selected: string | null, speedTurns: readonly ConstructionSpeedTurn[], units: Units): string {
  if (match.entrySpeed === null) return unreachableMessage;
  if (match.recommendedId === null) return "No turn banked for speed fits here.";
  const designSpeed = (id: string): number => {
    const turn = speedTurns.find((candidate) => candidate.id === id);
    if (!turn) throw new Error(`Missing speed-matched turn ${id}`);
    return turn.designSpeed;
  };
  const recommendedSpeed = designSpeed(match.recommendedId);
  const recommendedLabel = speedTurnSizeLabel(recommendedSpeed, units);
  // The force readout below prints the entry speed, so the advice does not repeat a second estimate of it.
  if (match.fits === false) {
    return match.entrySpeed > recommendedSpeed
      ? `The train is too fast for every turn banked for speed; the steepest is ${recommendedLabel.toLowerCase()}. Slow the train first with a hill or brakes.`
      : `The train is a little slow for these banks; ${recommendedLabel.toLowerCase()} is the closest.`;
  }
  const override = selected && selected !== match.recommendedId && speedTurns.some(({ id }) => id === selected)
    ? ` You selected the turn ${speedTurnSizeLabel(designSpeed(selected), units).toLowerCase()}.`
    : "";
  return `${recommendedLabel} suits the predicted entry speed.${override}`;
}
