import type { Camera, Viewport } from "../../camera";
import type { Atlas } from "../scene";
import type { CoasterColorScheme } from "../../coaster-colors";
import { buildTrackQuads, trackPaint, trackStyle, type TrackQuad } from "./geometry";

export function createStaticTrackCache(atlas: Atlas) {
  const empty = new Float32Array();
  let previous: { key: string; rails: Float32Array; ties: Float32Array; supports: Float32Array; quads: TrackQuad[] } | undefined;
  return (rails: Float32Array, ties: Float32Array, supports: Float32Array, colors: CoasterColorScheme, camera: Camera, viewport: Viewport): readonly TrackQuad[] => {
    const key = `${camera.centerX},${camera.centerY},${camera.zoom},${camera.rotation},${viewport.width},${viewport.height},${colors.track},${colors.supports},${colors.trains}`;
    if (!previous || previous.key !== key || previous.rails !== rails || previous.ties !== ties || previous.supports !== supports) {
      previous = { key, rails, ties, supports, quads: buildTrackQuads(rails, ties, supports, empty, camera, viewport, trackPaint(trackStyle(atlas), colors)) };
    }
    return previous.quads;
  };
}
