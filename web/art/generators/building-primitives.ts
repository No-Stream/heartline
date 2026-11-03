// Reusable construction details shared by the building designs.
import { type Vector3 } from "./iso.ts";
import { INK, shadePlum } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model, type Surface } from "./solid.ts";

export interface GableRoofGeometry {
  frontGable: Vector3[];
  backGable: Vector3[];
  frontSurface: [Vector3, Vector3, Vector3, Vector3];
  backSurface: [Vector3, Vector3, Vector3, Vector3];
  slopes: [[Vector3, Vector3, Vector3, Vector3], [Vector3, Vector3, Vector3, Vector3]];
}

export function gableRoofGeometry(
  half: number,
  edge: number,
  eaveHeight: number,
  ridgeHeight: number,
  gableRidgeInset: number,
  roofEaveDrop: number,
): GableRoofGeometry {
  return {
    frontGable: [[half, -half, eaveHeight], [half, half, eaveHeight], [half, 0, ridgeHeight - gableRidgeInset]],
    backGable: [[-half, half, eaveHeight], [-half, -half, eaveHeight], [-half, 0, ridgeHeight - gableRidgeInset]],
    frontSurface: [[half, -half, eaveHeight], [half, half, eaveHeight], [half, half, ridgeHeight], [half, -half, ridgeHeight]],
    backSurface: [[-half, half, eaveHeight], [-half, -half, eaveHeight], [-half, -half, ridgeHeight], [-half, half, ridgeHeight]],
    slopes: [
      [[edge, edge, eaveHeight - roofEaveDrop], [-edge, edge, eaveHeight - roofEaveDrop], [-edge, 0, ridgeHeight], [edge, 0, ridgeHeight]],
      [[-edge, -edge, eaveHeight - roofEaveDrop], [edge, -edge, eaveHeight - roofEaveDrop], [edge, 0, ridgeHeight], [-edge, 0, ridgeHeight]],
    ],
  };
}

/** A sloped striped awning that sticks out from a wall, with a scalloped front edge. */
export function awningOnFront(
  model: Model,
  frontX: number,
  centerY: number,
  awningWidth: number,
  wallHeight: number,
  reach: number,
  drop: number,
  stripeCount: number,
  firstColor: string,
  secondColor: string,
): void {
  const y0 = centerY - awningWidth / 2;
  const stripeWidth = awningWidth / stripeCount;
  const outerX = frontX + reach;
  const outerZ = wallHeight - drop;
  for (let index = 0; index < stripeCount; index++) {
    const [start, end] = [y0 + index * stripeWidth, y0 + (index + 1) * stripeWidth];
    const color = index % 2 ? secondColor : firstColor;
    model.face([[outerX, start, outerZ], [outerX, end, outerZ], [frontX, end, wallHeight], [frontX, start, wallHeight]], color, { ink: 0.9 });
    const hem: Vector3[] = [];
    const scallopDepth = model.theme.props.scallops ? 3 : 0;
    for (let step = 0; step <= 6; step++) {
      const fraction = step / 6;
      hem.push([outerX, start + (end - start) * (1 - fraction), outerZ - 2.2 - Math.sin(fraction * Math.PI) * scallopDepth]);
    }
    model.face([[outerX, start, outerZ], ...hem, [outerX, end, outerZ]].reverse() as Vector3[], shadePlum(color, 0.08), { ink: 0.9, lit: false, wobbleAmount: 0.05 });
  }
}

/** Thin vertical ribs across a band of a wall, as on a pressed metal or panelled skirt. */
export function ribbed(model: Model, surface: Surface, bottom: number, height: number, ribCount: number): void {
  for (let rib = 1; rib < ribCount; rib++) {
    const u = (surface.width * rib) / ribCount;
    model.lineOn(surface, [[u, bottom + 1], [u, bottom + height - 1]], 0.55, INK, 0.4);
  }
}

/** Standing seams running from the eave up to the ridge of a flat-sheeted roof slope. */
export function seamedRoof(model: Model, slope: Surface, seamCount: number): void {
  for (let seam = 1; seam < seamCount; seam++) {
    const u = (slope.width * seam) / seamCount;
    model.lineOn(slope, [[u, 0.5], [u, slope.height - 0.5]], 0.6, INK, 0.5);
  }
}

/** A square sign plate with a person pictogram on it, skirted or not. */
export function pictogramSign(model: Model, surface: Surface, centerU: number, bottom: number, skirted: boolean, figureColor: string): void {
  model.rect(surface, centerU - 3.8, bottom, 7.6, 9.6, COLORS.paper, 0.7);
  model.ellipseOn(surface, centerU, bottom + 7.4, 1.2, 1.2, figureColor, 0);
  model.paint(surface, skirted
    ? [[centerU - 2.4, bottom + 1.4], [centerU + 2.4, bottom + 1.4], [centerU + 1, bottom + 5.8], [centerU - 1, bottom + 5.8]]
    : [[centerU - 1.2, bottom + 1.4], [centerU + 1.2, bottom + 1.4], [centerU + 1.6, bottom + 5.8], [centerU - 1.6, bottom + 5.8]], figureColor, 0);
}
