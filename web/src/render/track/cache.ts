import type { Camera, Viewport } from "../../camera";
import { geometryIntersectsViewport, type Atlas } from "../scene";
import { buildTrackQuads, trackStyle, type TrackQuad } from "./geometry";

export function createStaticTrackCache(atlas: Atlas) {
  const empty = new Float32Array();
  let previous: { key: string; rails: Float32Array; ties: Float32Array; supports: Float32Array; quads: TrackQuad[] } | undefined;
  return (rails: Float32Array, ties: Float32Array, supports: Float32Array, camera: Camera, viewport: Viewport): readonly TrackQuad[] => {
    const key = `${camera.centerX},${camera.centerY},${camera.zoom},${camera.rotation},${viewport.width},${viewport.height}`;
    if (!previous || previous.key !== key || previous.rails !== rails || previous.ties !== ties || previous.supports !== supports) {
      previous = { key, rails, ties, supports, quads: buildTrackQuads(rails, ties, supports, empty, camera, viewport, trackStyle(atlas)).filter((quad) => geometryIntersectsViewport(quad, viewport)) };
    }
    return previous.quads;
  };
}
