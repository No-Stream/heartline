import type { Camera, Viewport } from "../../camera";
import { buildTrackQuads, type TrackQuad, type TrackStyle } from "./geometry";

/** Draw the worker's proposed track in green when placeable and red otherwise. */
export function buildGhostQuads(rails: Float32Array, ties: Float32Array, supports: Float32Array, valid: boolean, camera: Camera, viewport: Viewport, style: TrackStyle): TrackQuad[] {
  const color: TrackQuad["color"] = valid ? [0.15, 0.8, 0.35, 0.55] : [0.9, 0.2, 0.2, 0.55];
  return buildTrackQuads(rails, ties, supports, new Float32Array(), camera, viewport, style)
    .map((quad) => ({ ...quad, id: `preview-${quad.id}`, color }));
}
