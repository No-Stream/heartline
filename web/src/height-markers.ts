import type { WorldPoint } from "./camera";
import { terrainHeightAt, type WorldView } from "./render/scene";

export interface HeightMarker { readonly position: WorldPoint; readonly metres: number }
export interface HeightExtremum { readonly marker: HeightMarker; readonly kind: "crest" | "valley" }

const minimumExtremumChangeMetres = 3;

export function findHeightExtrema(samples: readonly HeightMarker[]): HeightExtremum[] {
  if (samples.length < 3) return [];

  const elevation = (sample: HeightMarker): number => sample.position.z * 4;
  const extrema: HeightExtremum[] = [];
  let previousExtremum = samples[0];
  let candidate = samples[0];
  let direction: "rising" | "falling" | null = null;

  for (const sample of samples.slice(1)) {
    const sampleElevation = elevation(sample);
    const candidateElevation = elevation(candidate);

    if (direction === null) {
      if (sampleElevation - candidateElevation >= minimumExtremumChangeMetres) {
        direction = "rising";
        candidate = sample;
      } else if (candidateElevation - sampleElevation >= minimumExtremumChangeMetres) {
        direction = "falling";
        candidate = sample;
      }
    } else if (direction === "rising") {
      if (sampleElevation >= candidateElevation) {
        candidate = sample;
      } else if (candidateElevation - sampleElevation >= minimumExtremumChangeMetres) {
        if (candidateElevation - elevation(previousExtremum) >= minimumExtremumChangeMetres) extrema.push({ marker: candidate, kind: "crest" });
        previousExtremum = candidate;
        candidate = sample;
        direction = "falling";
      }
    } else if (sampleElevation <= candidateElevation) {
      candidate = sample;
    } else if (sampleElevation - candidateElevation >= minimumExtremumChangeMetres) {
      if (elevation(previousExtremum) - candidateElevation >= minimumExtremumChangeMetres) extrema.push({ marker: candidate, kind: "valley" });
      previousExtremum = candidate;
      candidate = sample;
      direction = "rising";
    }
  }

  return extrema;
}

export function constructionHeights(world: WorldView, rails: Float32Array): { station: HeightMarker; end: HeightMarker; extrema: HeightExtremum[] } | null {
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
  const samples: HeightMarker[] = [];
  for (let offset = 0; offset < rails.length; offset += 38) {
    if (offset === 0) samples.push(marker(offset + 1));
    samples.push(marker(offset + 10));
  }
  return { station: samples[0], end: samples[samples.length - 1], extrema: findHeightExtrema(samples) };
}
