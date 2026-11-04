import type { ConstructionPiece, PieceShape } from "./construction-protocol";

export type StepAttribute = "turn" | "slope" | "bank";
export type PieceStep = StepAttribute | "piece";

export const pieceGroups = ["straight", "turn", "slope", "bank", "inversion", "special"] as const;

const slopeOrder = ["DownSixty", "DownSteep", "DownModerate", "DownGentle", "Level", "UpGentle", "UpModerate", "UpSteep", "UpSixty"];
const slopeLabels: Record<string, string> = {
  DownSixty: "Very steep down 60°", DownSteep: "Steep down 45°", DownModerate: "Moderate down 37°", DownGentle: "Gentle down 27°", Level: "Level",
  UpGentle: "Gentle up 27°", UpModerate: "Moderate up 37°", UpSteep: "Steep up 45°", UpSixty: "Very steep up 60°",
};
const steepCurveNames: Record<string, string> = {
  "Begin upward curve": "Start of steep upward curve",
  "Continue upward curve": "Middle of steep upward curve",
  "End upward curve": "End of steep upward curve",
  "Begin downward curve": "Start of steep downward curve",
  "Continue downward curve": "Middle of steep downward curve",
  "End downward curve": "End of steep downward curve",
};
const bankOrder = ["Left", "Level", "Right"];

function turnLabel(turn: number): string {
  if (turn === 0) return "Ahead";
  return `${turn > 0 ? "Left" : "Right"} ${Math.abs(turn) * 45}°`;
}

function normalizedTurnName(label: string): string {
  const repeatedBankDirection = /^(Begin |End )?(small|large) (left|right)-banked \3 turn$/.exec(label);
  if (!repeatedBankDirection) return label;
  return `${repeatedBankDirection[1] ?? ""}${repeatedBankDirection[2]} ${repeatedBankDirection[3]}-banked turn`;
}

/** A piece's stepper value, ordered so that a lower rank sits on the "previous" side of the stepper. */
function attribute(piece: ConstructionPiece, shapes: ReadonlyMap<string, PieceShape>, name: StepAttribute): { rank: number; label: string } {
  switch (name) {
    case "turn": {
      const shape = shapes.get(piece.id);
      if (!shape) throw new Error(`Missing shape for piece ${piece.id}`);
      return { rank: -shape.turn, label: turnLabel(shape.turn) };
    }
    case "slope": return { rank: slopeOrder.indexOf(piece.slope), label: slopeLabels[piece.slope] ?? piece.slope };
    case "bank": return { rank: bankOrder.indexOf(piece.bank), label: piece.bank };
  }
}

const stepAttributes: readonly StepAttribute[] = ["turn", "slope", "bank"];

/** Pieces that differ from the selected one only in the given attribute, with that attribute's distinct values in stepper order. */
function stepperValues(pieces: readonly ConstructionPiece[], shapes: ReadonlyMap<string, PieceShape>, selected: ConstructionPiece, name: StepAttribute) {
  const fixed = stepAttributes.filter((other) => other !== name);
  const candidates = pieces.filter((candidate) => fixed.every((other) => attribute(candidate, shapes, other).rank === attribute(selected, shapes, other).rank));
  const ranks = [...new Set(candidates.map((candidate) => attribute(candidate, shapes, name).rank))].sort((first, second) => first - second);
  return { candidates, ranks };
}

export function stepperOptions(pieces: readonly ConstructionPiece[], shapes: ReadonlyMap<string, PieceShape>, selectedId: string | null, name: StepAttribute): { values: string[]; index: number } | undefined {
  const selected = pieces.find(({ id }) => id === selectedId);
  if (!selected) return undefined;
  const { candidates, ranks } = stepperValues(pieces, shapes, selected, name);
  const values = ranks.map((rank) => attribute(candidates.find((candidate) => attribute(candidate, shapes, name).rank === rank)!, shapes, name).label);
  return { values, index: ranks.indexOf(attribute(selected, shapes, name).rank) };
}

/** Palette order: by group, then in the worker's catalog order. */
export function groupedPieces(pieces: readonly ConstructionPiece[]): { group: string; pieces: ConstructionPiece[] }[] {
  const unknown = pieces.find(({ group }) => !(pieceGroups as readonly string[]).includes(group));
  if (unknown) throw new Error(`Unknown piece group ${unknown.group}`);
  return pieceGroups.map((group) => ({ group, pieces: pieces.filter((piece) => piece.group === group) })).filter((entry) => entry.pieces.length > 0);
}

export function pieceDisplayName(piece: Pick<ConstructionPiece, "group" | "label" | "slope">): string {
  if (piece.group === "slope" && steepCurveNames[piece.label]) return steepCurveNames[piece.label];
  if (piece.group !== "turn") return piece.label;
  const name = normalizedTurnName(piece.label.replace(/^Standalone /, ""));
  const prefix = piece.slope.startsWith("Up") ? "Climbing" : piece.slope.startsWith("Down") ? "Descending" : "";
  const phase = /^(Begin|End) (.+)$/.exec(name);
  if (phase) {
    const beginningOrEnd = phase[1] === "Begin" ? "Start" : "End";
    const prefixText = prefix ? `${prefix.toLowerCase()} ` : "";
    return `${beginningOrEnd} of a ${prefixText}${phase[2]}`;
  }
  if (!prefix) return `${name.charAt(0).toUpperCase()}${name.slice(1)}`;
  return `${prefix} ${name.charAt(0).toLowerCase()}${name.slice(1)}`;
}

/**
 * The piece one step away from the selection: along one attribute (clamped at the ends, preferring placeable
 * pieces of the same kind, such as staying on the chain lift) or through the whole palette (wrapping).
 */
export function steppedPiece(pieces: readonly ConstructionPiece[], shapes: ReadonlyMap<string, PieceShape>, selectedId: string | null, name: PieceStep, step: -1 | 1): string | undefined {
  const selected = pieces.find(({ id }) => id === selectedId);
  if (!selected) return pieceInGroup(pieces, "straight") ?? pieces[0]?.id;
  if (name === "piece") {
    const palette = groupedPieces(pieces).flatMap((entry) => entry.pieces);
    return palette[(palette.indexOf(selected) + step + palette.length) % palette.length].id;
  }
  const { candidates, ranks } = stepperValues(pieces, shapes, selected, name);
  const nextRank = ranks[ranks.indexOf(attribute(selected, shapes, name).rank) + step];
  if (nextRank === undefined) return undefined;
  const kind = shapes.get(selected.id)?.kind;
  const preference = (candidate: ConstructionPiece) => (candidate.refusal ? 2 : 0) + (shapes.get(candidate.id)?.kind === kind ? 0 : 1);
  const matches = candidates.filter((candidate) => attribute(candidate, shapes, name).rank === nextRank);
  return matches.reduce((best, candidate) => preference(candidate) < preference(best) ? candidate : best).id;
}

export function pieceInGroup(pieces: readonly ConstructionPiece[], group: string): string | undefined {
  const members = pieces.filter((piece) => piece.group === group);
  return (members.find((piece) => !piece.refusal) ?? members[0])?.id;
}
