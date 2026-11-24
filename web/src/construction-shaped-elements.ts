import type { ConstructionShapedElementWire } from "./generated/ConstructionShapedElementWire";

export type ShapedHillFeel = "floater" | "strongFloater" | "ejector";
export type ShapedLoopTopFeel = "floaty" | "neutral" | "pressedIn";

export type ShapedElementSelection =
  | {
      readonly kind: "hill";
      readonly feel: ShapedHillFeel;
      readonly strengthG: number;
      readonly size: { readonly kind: "auto" } | { readonly kind: "height"; readonly heightMetres: number };
    }
  | {
      readonly kind: "turn";
      readonly hand: "left" | "right";
      readonly feel: "fullyBanked";
      readonly size: { readonly kind: "auto" } | { readonly kind: "radius"; readonly radiusMetres: number };
    }
  | {
      readonly kind: "loop";
      readonly hand: "left" | "right";
      readonly peakG: number;
      readonly topFeel: ShapedLoopTopFeel;
    }
  | {
      readonly kind: "zeroGRoll";
      readonly hand: "left" | "right";
      readonly peakG: number;
    }
  | {
      readonly kind: "ravenTurn";
      readonly hand: "left" | "right";
      readonly peakG: number;
      readonly topFeel: ShapedLoopTopFeel;
    };

function fixedHillHeight(size: Extract<ShapedElementSelection, { kind: "hill" }>['size']): string {
  if (size.kind === "auto") return "auto";
  if (!Number.isFinite(size.heightMetres) || size.heightMetres <= 0) {
    throw new RangeError(`Invalid shaped hill height ${size.heightMetres}`);
  }
  return `fixed-${size.heightMetres}`;
}

function fixedTurnRadius(size: Extract<ShapedElementSelection, { kind: "turn" }>['size']): string {
  if (size.kind === "auto") return "auto";
  if (!Number.isFinite(size.radiusMetres) || size.radiusMetres <= 0) {
    throw new RangeError(`Invalid shaped turn radius ${size.radiusMetres}`);
  }
  return `fixed-${size.radiusMetres}`;
}

export function shapeSelectionId(selection: ShapedElementSelection): string {
  if (selection.kind === "hill") {
    if (!Number.isFinite(selection.strengthG)) {
      throw new RangeError(`Invalid shaped hill strength ${selection.strengthG}`);
    }
    return `shape:hill:${selection.feel}:${selection.strengthG}:${fixedHillHeight(selection.size)}`;
  }
  if (selection.kind === "turn") {
    return `shape:turn:${selection.hand}:${selection.feel}:${fixedTurnRadius(selection.size)}`;
  }
  if (!Number.isFinite(selection.peakG) || selection.peakG <= 0) {
    throw new RangeError(`Invalid shaped inversion peak g ${selection.peakG}`);
  }
  if (selection.kind === "loop") {
    return `shape:loop:${selection.hand}:${selection.peakG}:${selection.topFeel}`;
  }
  if (selection.kind === "zeroGRoll") {
    return `shape:zeroGRoll:${selection.hand}:${selection.peakG}`;
  }
  return `shape:ravenTurn:${selection.hand}:${selection.peakG}:${selection.topFeel}`;
}

export function defaultShapeSelectionId(feel: ShapedHillFeel): string {
  return `shape:hill:${feel}:default:auto`;
}

export function defaultInversionSelectionId(
  kind: "loop" | "zeroGRoll" | "ravenTurn",
  hand: "left" | "right",
): string {
  if (kind === "zeroGRoll") return `shape:zeroGRoll:${hand}:default`;
  return `shape:${kind}:${hand}:default:neutral`;
}

const hillFeels: readonly ShapedHillFeel[] = ["floater", "strongFloater", "ejector"];
const loopTopFeels: readonly ShapedLoopTopFeel[] = ["floaty", "neutral", "pressedIn"];
const loopPeakSteps: readonly number[] = [3.5, 3.8, 4.0, 4.2, 4.5];
const zeroGRollPeakSteps: readonly number[] = [3.0, 3.5, 3.8, 4.0, 4.2, 4.5];

export type ShapedStepAttribute = "feel" | "strength" | "size" | "radius" | "peakG" | "topFeel";

function parseTopFeel(value: string | null): ShapedLoopTopFeel {
  if (value === "floaty" || value === "neutral" || value === "pressedIn") return value;
  throw new Error(`Unknown shaped loop top feel ${value}`);
}

/** The selection the worker reports for its selected shaped element. */
export function shapedElementSelection(shape: ConstructionShapedElementWire): ShapedElementSelection {
  if (shape.kind === "hill") {
    if (shape.feel !== "floater" && shape.feel !== "strongFloater" && shape.feel !== "ejector") {
      throw new Error(`Unknown shaped hill feel ${shape.feel}`);
    }
    if (shape.strengthG === null) throw new Error("Selected shaped hill has no explicit target g");
    return {
      kind: "hill",
      feel: shape.feel,
      strengthG: shape.strengthG,
      size: shape.fixedSize === null ? { kind: "auto" } : { kind: "height", heightMetres: shape.fixedSize },
    };
  }
  if (shape.hand !== "left" && shape.hand !== "right") throw new Error(`Unknown shaped element hand ${shape.hand}`);
  if (shape.kind === "turn") {
    return {
      kind: "turn",
      hand: shape.hand,
      feel: "fullyBanked",
      size: shape.fixedSize === null ? { kind: "auto" } : { kind: "radius", radiusMetres: shape.fixedSize },
    };
  }
  if (shape.strengthG === null) throw new Error(`Selected shaped ${shape.kind} has no explicit peak g`);
  if (shape.kind === "zeroGRoll") {
    return { kind: "zeroGRoll", hand: shape.hand, peakG: shape.strengthG };
  }
  const topFeel = parseTopFeel(shape.feel);
  if (shape.kind === "loop") {
    return { kind: "loop", hand: shape.hand, peakG: shape.strengthG, topFeel };
  }
  return { kind: "ravenTurn", hand: shape.hand, peakG: shape.strengthG, topFeel };
}

function stepPeakG(current: number, steps: readonly number[], direction: -1 | 1): number | null {
  if (direction > 0) {
    const next = steps.find((candidate) => candidate > current + 1e-6);
    return next ?? null;
  }
  for (let index = steps.length - 1; index >= 0; index -= 1) {
    if (steps[index] < current - 1e-6) return steps[index];
  }
  return null;
}

/** The selection one shaped-element stepper click asks for, or null when that click changes nothing. */
export function steppedShapedSelection(
  shape: ConstructionShapedElementWire,
  attribute: ShapedStepAttribute,
  step: -1 | 1,
): ShapedElementSelection | null {
  const selection = shapedElementSelection(shape);
  if (attribute === "feel" && selection.kind === "hill") {
    const index = hillFeels.indexOf(selection.feel);
    const next = hillFeels[Math.max(0, Math.min(hillFeels.length - 1, index + step))];
    return next === selection.feel ? null : { ...selection, feel: next };
  }
  if (attribute === "strength" && selection.kind === "hill") {
    // Step exactly 0.1 g from the current target; rounding to hundredths only removes float noise,
    // so off-grid defaults such as the −0.85 g strong floater stay reachable (−0.95 g, −1.05 g).
    const strengthG = Math.round((selection.strengthG - step * 0.1) * 100) / 100;
    return { ...selection, strengthG };
  }
  if (attribute === "size" && selection.kind === "hill") {
    const currentHeight = selection.size.kind === "height" ? selection.size.heightMetres : shape.spec?.heightM;
    if (currentHeight === null || currentHeight === undefined) return null;
    const heightMetres = Math.max(2, Math.round(currentHeight / 2) * 2 + step * 2);
    return { ...selection, size: { kind: "height", heightMetres } };
  }
  if (attribute === "radius" && selection.kind === "turn") {
    const currentRadius = selection.size.kind === "radius" ? selection.size.radiusMetres : shape.spec?.radiusM;
    if (currentRadius === null || currentRadius === undefined) return null;
    const radiusMetres = Math.max(2, Math.round(currentRadius / 2) * 2 + step * 2);
    return { ...selection, size: { kind: "radius", radiusMetres } };
  }
  if (attribute === "peakG" && (selection.kind === "loop" || selection.kind === "zeroGRoll" || selection.kind === "ravenTurn")) {
    const steps = selection.kind === "zeroGRoll" ? zeroGRollPeakSteps : loopPeakSteps;
    const peakG = stepPeakG(selection.peakG, steps, step);
    return peakG === null ? null : { ...selection, peakG };
  }
  if (attribute === "topFeel" && (selection.kind === "loop" || selection.kind === "ravenTurn")) {
    const index = loopTopFeels.indexOf(selection.topFeel);
    const next = loopTopFeels[Math.max(0, Math.min(loopTopFeels.length - 1, index + step))];
    return next === selection.topFeel ? null : { ...selection, topFeel: next };
  }
  return null;
}
