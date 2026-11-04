import type { Camera, Viewport } from "../../camera";
import { buildTrackQuads, type TrackPaint, type TrackQuad, type TrackStyle } from "./geometry";

/** Draw the worker's proposed track in a pale teal ink when placeable and coral otherwise. */
export function buildGhostQuads(rails: Float32Array, ties: Float32Array, supports: Float32Array, valid: boolean, camera: Camera, viewport: Viewport, style: TrackStyle): TrackQuad[] {
  const color: TrackQuad["color"] = valid ? [0.45, 0.86, 0.8, 0.62] : [0.9, 0.3, 0.2, 0.62];
  // Every quad is recoloured below, so the shape is drawn in the key line alone.
  const shape: TrackPaint = { track: style.ink, supports: style.ink, trains: style.ink, ink: style.ink, lineWeight: style.lineWeight };
  return buildTrackQuads(rails, ties, supports, camera, viewport, shape)
    .map((quad) => ({ ...quad, id: `preview-${quad.id}`, color }));
}
