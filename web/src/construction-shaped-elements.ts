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
