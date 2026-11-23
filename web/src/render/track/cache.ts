import type { Camera, Viewport } from "../../camera";
import { geometryIntersectsViewport, type Atlas } from "../scene";
import type { CoasterColorScheme } from "../../coaster-colors";
import { buildTrackQuads, trackPaint, trackStyle, type TrackQuad } from "./geometry";

export function createStaticTrackCache(atlas: Atlas) {
  const caches = new WeakMap<Float32Array, Map<number | undefined, { key: string; ties: Float32Array; supports: Float32Array; quads: TrackQuad[] }>>();
  return (rails: Float32Array, ties: Float32Array, supports: Float32Array, colors: CoasterColorScheme, camera: Camera, viewport: Viewport, coasterId?: number, trackRevision?: string): readonly TrackQuad[] => {
    let coasterCaches = caches.get(rails);
    if (!coasterCaches) {
      coasterCaches = new Map();
      caches.set(rails, coasterCaches);
    }
    const key = `${trackRevision ?? ""},${camera.centerX},${camera.centerY},${camera.zoom},${camera.rotation},${viewport.width},${viewport.height},${colors.track},${colors.supports},${colors.trains}`;
    const previous = coasterCaches.get(coasterId);
    if (!previous || previous.key !== key || previous.ties !== ties || previous.supports !== supports) {
      coasterCaches.set(coasterId, { key, ties, supports, quads: buildTrackQuads(rails, ties, supports, camera, viewport, trackPaint(trackStyle(atlas), colors), undefined, undefined, coasterId).filter((quad) => geometryIntersectsViewport(quad, viewport)) });
    }
    return coasterCaches.get(coasterId)!.quads;
  };
}
