import type { Vertex } from "./scene";

/**
 * A round window, in canvas pixels, through which the world is drawn cut at `level` (height steps) while the rest of the
 * view keeps the surface. Geometry carries a `CutSide` so the shader and the CPU picker agree on what shows where.
 */
export interface Cutaway { readonly x: number; readonly y: number; readonly radius: number; readonly level: number }
export interface ScreenBounds { readonly left: number; readonly top: number; readonly right: number; readonly bottom: number }

export const CutSide = { everywhere: 0, outside: 1, inside: 2 } as const;
export type CutSide = (typeof CutSide)[keyof typeof CutSide];

/** Radius of the cutaway window in CSS pixels. */
export const CUTAWAY_RADIUS = 150;
/** How far above the track end the cutaway cuts, in height steps, so the track and a train on it stay in view. */
export const CUTAWAY_TRACK_CLEARANCE = 3;

export function insideCutaway(cutaway: Cutaway, x: number, y: number): boolean {
  return Math.hypot(x - cutaway.x, y - cutaway.y) < cutaway.radius;
}

/** Whether a side's geometry is drawn at a canvas point. */
export function cutSideVisibleAt(side: CutSide, cutaway: Cutaway | undefined, x: number, y: number): boolean {
  if (!cutaway || side === CutSide.everywhere) return true;
  return insideCutaway(cutaway, x, y) === (side === CutSide.inside);
}

export function cutawayBounds(cutaway: Cutaway): ScreenBounds {
  return { left: cutaway.x - cutaway.radius, top: cutaway.y - cutaway.radius, right: cutaway.x + cutaway.radius, bottom: cutaway.y + cutaway.radius };
}

/** Conservative: true for every polygon that can reach into the window. */
export function polygonTouchesCutaway(vertices: readonly Vertex[], cutaway: Cutaway): boolean {
  const left = Math.min(...vertices.map((vertex) => vertex.x)); const right = Math.max(...vertices.map((vertex) => vertex.x));
  const top = Math.min(...vertices.map((vertex) => vertex.y)); const bottom = Math.max(...vertices.map((vertex) => vertex.y));
  const nearestX = Math.max(left, Math.min(cutaway.x, right)); const nearestY = Math.max(top, Math.min(cutaway.y, bottom));
  return Math.hypot(nearestX - cutaway.x, nearestY - cutaway.y) < cutaway.radius;
}

/** The cut level that keeps a track end at `z` (tile units) and the space just above it open. */
export function cutawayLevelForTrack(z: number): number {
  if (!Number.isFinite(z)) throw new RangeError("Track height must be finite");
  return Math.floor(z * 4) + CUTAWAY_TRACK_CLEARANCE;
}
