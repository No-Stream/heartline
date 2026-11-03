// The one projection shared by every drawing: 2:1 dimetric with tile edges 64x32 pixels on screen.
// World units are chosen so that one unit of height or depth is about one sprite pixel.
import type { Point } from "./ink.ts";

export type Vector3 = readonly [number, number, number];

export const TILE_SCREEN_WIDTH = 64;
export const TILE_SCREEN_HEIGHT = 32;
const AXIS_X = Math.SQRT1_2;
const AXIS_Y = Math.SQRT1_2 / 2;
const AXIS_Z = Math.sqrt(3) / 2;
/** World length of one tile edge. */
export const TILE_EDGE = TILE_SCREEN_WIDTH / 2 / AXIS_X;

export function project([x, y, z]: Vector3): Point {
  return [(x - y) * AXIS_X, (x + y) * AXIS_Y - z * AXIS_Z];
}

/** Larger values are closer to the viewer. */
export function depthOf([x, y, z]: Vector3): number {
  return (x + y) * 0.612 + z * 0.5;
}

/** Rotates a point by quarter turns about the vertical axis; one turn moves screen south-east to south-west. */
export function rotateQuarter([x, y, z]: Vector3, quarterTurns: number): Vector3 {
  switch (((quarterTurns % 4) + 4) % 4) {
    case 0:
      return [x, y, z];
    case 1:
      return [-y, x, z];
    case 2:
      return [-x, -y, z];
    default:
      return [y, -x, z];
  }
}
