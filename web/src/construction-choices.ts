import type { ConstructionHillOption, ConstructionPiece, ConstructionSpeedTurn, PieceShape } from "./construction-protocol";
import { formatLength, metresToDisplay, unitLabel, type Units } from "./units";

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
const bankOrder = ["Left", "Level", "Right", "Inverted"];
const bankLabels: Record<string, string> = { Left: "Left 45°", Level: "Level", Right: "Right 45°", Inverted: "Inverted" };
/** The stepper's own bank value for a speed-matched turn, whose connectors are level but whose bank follows the curve. */
const speedBank = "ForSpeed";
/** From this predicted entry speed (m/s) a turn step selects the speed-matched turn; slower trains get the plain turn. */
export const speedTurnDefaultMinimumSpeed = 12;

/**
 * The pieces the steppers choose from: speed-matched turns carry the "For speed" bank, and each hand the palette
 * does not list (it shows only the selected family variant) gets a stand-in that the worker sizes for the speed.
 */
export function speedTurnPalette(pieces: readonly ConstructionPiece[], speedTurns: readonly ConstructionSpeedTurn[]): ConstructionPiece[] {
  const speedTurnIds = new Set(speedTurns.map(({ id }) => id));
  const listed = pieces.map((piece) => speedTurnIds.has(piece.id) ? { ...piece, bank: speedBank } : piece);
  const listedHands = new Set(speedTurns.filter(({ id }) => pieces.some((piece) => piece.id === id)).map(({ hand }) => hand));
  const standIns = (["left", "right"] as const).flatMap((hand) => {
    if (listedHands.has(hand)) return [];
    const slowest = speedTurns.filter((turn) => turn.hand === hand).sort((first, second) => first.designSpeed - second.designSpeed)[0];
    return slowest ? [{
      id: slowest.id, label: slowest.id, group: "turn", heading: "", slope: "Level", bank: speedBank,
      refusal: "", wouldTunnel: false, heartlined: null, hillEstimate: null,
    }] : [];
  });
  return [...listed, ...standIns];
}

function shapeOf(shapes: ReadonlyMap<string, PieceShape>, id: string): PieceShape {
  const shape = shapes.get(id);
  if (!shape) throw new Error(`Missing shape for piece ${id}`);
  return shape;
}

/** A speed-matched turn leans into its curve, so it sits between level and the 45° bank on the turn's side. */
function bankRank(piece: ConstructionPiece, shapes: ReadonlyMap<string, PieceShape>): number {
  if (piece.bank !== speedBank) return bankOrder.indexOf(piece.bank);
  return bankOrder.indexOf("Level") + (shapeOf(shapes, piece.id).turn > 0 ? -0.5 : 0.5);
}

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
      const shape = shapeOf(shapes, piece.id);
      return { rank: -shape.turn, label: turnLabel(shape.turn) };
    }
    case "slope": return { rank: slopeOrder.indexOf(piece.slope), label: slopeLabels[piece.slope] ?? piece.slope };
    case "bank": return { rank: bankRank(piece, shapes), label: piece.bank === speedBank ? "For speed" : bankLabels[piece.bank] ?? piece.bank };
  }
}

/** When another attribute steps, a speed-matched turn counts as level track, so its direction and slope can change. */
function heldRank(piece: ConstructionPiece, shapes: ReadonlyMap<string, PieceShape>, name: StepAttribute): number {
  return name === "bank" && piece.bank === speedBank ? bankOrder.indexOf("Level") : attribute(piece, shapes, name).rank;
}

const stepAttributes: readonly StepAttribute[] = ["turn", "slope", "bank"];

/** Pieces that differ from the selected one only in the given attribute, with that attribute's distinct values in stepper order. */
function stepperValues(pieces: readonly ConstructionPiece[], shapes: ReadonlyMap<string, PieceShape>, selected: ConstructionPiece, name: StepAttribute) {
  const fixed = stepAttributes.filter((other) => other !== name);
  const candidates = pieces.filter((candidate) => fixed.every((other) => heldRank(candidate, shapes, other) === heldRank(selected, shapes, other)));
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

function exitPoint(shape: PieceShape | undefined): readonly [number, number] {
  const last = shape?.points.at(-1);
  return last ? [last[0], last[1]] : [0, 0];
}

/**
 * The piece one step away from the selection: along one attribute (clamped at the ends, preferring placeable
 * pieces of the same kind, such as staying on the chain lift) or through the whole palette (wrapping). A step
 * onto a turn takes the speed-matched turn when the predicted entry speed is high enough, and a bank step keeps
 * the turn's footprint as close as it can.
 */
export function steppedPiece(pieces: readonly ConstructionPiece[], shapes: ReadonlyMap<string, PieceShape>, selectedId: string | null, name: PieceStep, step: -1 | 1, entrySpeed?: number): string | undefined {
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
  const preferSpeedBank = entrySpeed !== undefined && entrySpeed >= speedTurnDefaultMinimumSpeed;
  const [selectedX, selectedY] = exitPoint(shapes.get(selected.id));
  const cost = (candidate: ConstructionPiece): number[] => {
    const [x, y] = exitPoint(shapes.get(candidate.id));
    return [
      (candidate.bank === speedBank) === preferSpeedBank ? 0 : 1,
      candidate.refusal ? 1 : 0,
      shapes.get(candidate.id)?.kind === kind ? 0 : 1,
      name === "bank" ? Math.hypot(x - selectedX, y - selectedY) : 0,
    ];
  };
  const cheaper = (first: number[], second: number[]): boolean => {
    const difference = first.findIndex((value, index) => value !== second[index]);
    return difference >= 0 && first[difference] < second[difference];
  };
  const matches = candidates.filter((candidate) => attribute(candidate, shapes, name).rank === nextRank);
  return matches.reduce((best, candidate) => cheaper(cost(candidate), cost(best)) ? candidate : best).id;
}

export function pieceInGroup(pieces: readonly ConstructionPiece[], group: string): string | undefined {
  const members = pieces.filter((piece) => piece.group === group);
  return (members.find((piece) => !piece.refusal) ?? members[0])?.id;
}

/**
 * A step names an exact palette entry, so speed matching must not redirect navigation; only a step onto a
 * speed-matched turn lets the worker pick the size for the predicted entry speed.
 */
export function steppedPieceSelection(pieces: readonly ConstructionPiece[], shapes: ReadonlyMap<string, PieceShape>, selectedId: string | null, name: PieceStep, step: -1 | 1, entrySpeed: number | undefined): { type: "construction-select"; piece: string; exact: boolean } | undefined {
  const piece = steppedPiece(pieces, shapes, selectedId, name, step, entrySpeed);
  if (!piece || piece === selectedId) return undefined;
  return { type: "construction-select", piece, exact: pieces.find(({ id }) => id === piece)?.bank !== speedBank };
}

export type HillAttribute = "feel" | "height" | "span" | "twist";

const hillFeelOrder = ["ejector", "strongFloater", "floater", "speed", "zeroGRoll", "stall"];
/** Left-hand twists sit on the "previous" side, as on the bank stepper. */
const hillTwistOrder = ["outerLeft", "none", "outerRight", "left", "right"];
export const hillFeelLabels: Readonly<Record<string, string>> = { ejector: "Ejector", strongFloater: "Strong floater", floater: "Floater", speed: "Speed hill", zeroGRoll: "Zero-g roll", stall: "Stall" };
export const hillTwistLabels: Readonly<Record<string, string>> = { none: "Upright", outerLeft: "Outer bank left", outerRight: "Outer bank right", left: "Roll left", right: "Roll right" };

function orderRank(order: readonly string[], value: string): number {
  const rank = order.indexOf(value);
  if (rank < 0) throw new Error(`Unknown hill value ${value}`);
  return rank;
}

function hillRank(option: ConstructionHillOption, attribute: HillAttribute): number {
  switch (attribute) {
    case "feel": return orderRank(hillFeelOrder, option.feel);
    case "twist": return orderRank(hillTwistOrder, option.twist);
    case "height": return option.height;
    case "span": return option.span;
  }
}

/** Every feel is one step away; twists are those of the selected feel; sizes are those of the selected feel and twist. */
function hillCandidates(options: readonly ConstructionHillOption[], selected: ConstructionHillOption, attribute: HillAttribute): ConstructionHillOption[] {
  if (attribute === "feel") return [...options];
  if (attribute === "twist") return options.filter((option) => option.feel === selected.feel);
  return options.filter((option) => option.feel === selected.feel && option.twist === selected.twist);
}

function hillRanks(candidates: readonly ConstructionHillOption[], attribute: HillAttribute): number[] {
  return [...new Set(candidates.map((option) => hillRank(option, attribute)))].sort((first, second) => first - second);
}

function twistSide(twist: string): string {
  return twist === "outerLeft" || twist === "left" ? "left" : twist === "outerRight" || twist === "right" ? "right" : "none";
}

/** The option most like the reference: the same twist (or at least the same side), then the closest height, then the closest length. */
function nearestHill(candidates: readonly ConstructionHillOption[], reference: ConstructionHillOption): ConstructionHillOption {
  const cost = (option: ConstructionHillOption): number[] => [
    option.twist === reference.twist ? 0 : twistSide(option.twist) === twistSide(reference.twist) ? 1 : 2,
    Math.abs(option.height - reference.height),
    Math.abs(option.span - reference.span),
  ];
  const cheaper = (first: number[], second: number[]): boolean => {
    const difference = first.findIndex((value, index) => value !== second[index]);
    return difference >= 0 && first[difference] < second[difference];
  };
  return candidates.reduce((best, option) => cheaper(cost(option), cost(best)) ? option : best);
}

export function hillLabel(option: Pick<ConstructionHillOption, "feel" | "height" | "span">, units: Units): string {
  return `${hillFeelLabels[option.feel] ?? option.feel} · ${formatHillSize(option.height, option.span, units)}`;
}

export function formatHillSize(height: number, span: number, units: Units): string {
  const value = (metres: number): string => new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(metresToDisplay(metres, units));
  return `${value(height)} × ${value(span)} ${unitLabel(units)}`;
}

function hillValueLabel(option: ConstructionHillOption, attribute: HillAttribute, units: Units): string {
  switch (attribute) {
    case "feel": return hillFeelLabels[option.feel] ?? option.feel;
    case "twist": return hillTwistLabels[option.twist] ?? option.twist;
    case "height": return formatLength(option.height, units, 0);
    case "span": return formatLength(option.span, units, 0);
  }
}

export function hillStepperOptions(options: readonly ConstructionHillOption[], selectedId: string | null, attribute: HillAttribute, units: Units): { values: string[]; index: number } | undefined {
  const selected = options.find(({ id }) => id === selectedId);
  if (!selected) return undefined;
  const candidates = hillCandidates(options, selected, attribute);
  const ranks = hillRanks(candidates, attribute);
  const values = ranks.map((rank) => hillValueLabel(candidates.find((option) => hillRank(option, attribute) === rank)!, attribute, units));
  return { values, index: ranks.indexOf(hillRank(selected, attribute)) };
}

/**
 * The selection one hill stepper step away, clamped at the ends. A new feel or twist is a new kind of hill, so the
 * worker picks its size for the predicted entry speed; a height or length step is the player's own size and is exact.
 */
export function steppedHillSelection(options: readonly ConstructionHillOption[], selectedId: string | null, attribute: HillAttribute, step: -1 | 1): { type: "construction-select"; piece: string; exact: boolean } | undefined {
  const selected = options.find(({ id }) => id === selectedId);
  if (!selected) return undefined;
  const candidates = hillCandidates(options, selected, attribute);
  const ranks = hillRanks(candidates, attribute);
  const nextRank = ranks[ranks.indexOf(hillRank(selected, attribute)) + step];
  if (nextRank === undefined) return undefined;
  const next = nearestHill(candidates.filter((option) => hillRank(option, attribute) === nextRank), selected);
  return { type: "construction-select", piece: next.id, exact: attribute === "height" || attribute === "span" };
}

/** The hill to select when the player opens the hill section: the current variant, or the most similar hill of the preferred feel. */
export function hillToOpen(options: readonly ConstructionHillOption[], pieces: readonly ConstructionPiece[], preferredFeel: string | null): string | undefined {
  const current = options.find((option) => pieces.some(({ id }) => id === option.id)) ?? options[0];
  if (!current) return undefined;
  const preferred = options.filter((option) => option.feel === preferredFeel);
  return (preferred.length > 0 ? nearestHill(preferred, current) : current).id;
}
