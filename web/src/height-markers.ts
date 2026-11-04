import type { WorldPoint } from "./camera";
import { terrainHeightAt, type WorldView } from "./render/scene";

interface HeightMarker { readonly position: WorldPoint; readonly metres: number }

export function constructionHeights(world: WorldView, rails: Float32Array): { station: HeightMarker; end: HeightMarker } | null {
  if (!rails.length) return null;
  if (rails.length % 38 !== 0) throw new Error("Expected paired coaster rails");
  const marker = (offset: number): HeightMarker => {
    const position = {
      x: (rails[offset] + rails[offset + 19]) / 2,
      y: (rails[offset + 1] + rails[offset + 20]) / 2,
      z: (rails[offset + 2] + rails[offset + 21]) / 2,
    };
    return { position, metres: position.z * 4 - terrainHeightAt(world, position.x, position.y) };
  };
  return { station: marker(1), end: marker(rails.length - 38 + 10) };
}
