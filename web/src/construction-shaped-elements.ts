import type { ConstructionShapedElementWire } from "./generated/ConstructionShapedElementWire";

export type ShapedHillFeel = "floater" | "strongFloater" | "ejector";

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
  return `shape:turn:${selection.hand}:${selection.feel}:${fixedTurnRadius(selection.size)}`;
}

export function defaultShapeSelectionId(feel: ShapedHillFeel): string {
  return `shape:hill:${feel}:default:auto`;
}

const hillFeels: readonly ShapedHillFeel[] = ["floater", "strongFloater", "ejector"];

export type ShapedStepAttribute = "feel" | "strength" | "size" | "radius";

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
  if (shape.hand !== "left" && shape.hand !== "right") throw new Error(`Unknown shaped turn hand ${shape.hand}`);
  return {
    kind: "turn",
    hand: shape.hand,
    feel: "fullyBanked",
    size: shape.fixedSize === null ? { kind: "auto" } : { kind: "radius", radiusMetres: shape.fixedSize },
  };
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
  return null;
}
