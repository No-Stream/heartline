import { describe, expect, it } from "vitest";
import { constructionHeights, findHeightExtrema } from "./height-markers";
import type { WorldView } from "./render/scene";

function rail(start: number[], end: number[]): number[] {
  return [0, ...start, 0, 1, 0, 0, 0, 1, ...end, 0, 1, 0, 0, 0, 1];
}

describe("construction height markers", () => {
  it("uses the track centre and interpolates the terrain under each marker", () => {
    const world = { dimensions: new Uint16Array([3, 2]), surfaces: new Int32Array([
      2, 4, 6, 4, 0, 0, 4, 6, 8, 6, 0, 0, 6, 8, 10, 8, 0, 0,
      4, 6, 8, 6, 0, 0, 6, 8, 10, 8, 0, 0, 8, 10, 12, 10, 0, 0,
    ]) } as WorldView;
    const rails = new Float32Array([
      ...rail([0.25, 0.15, 3], [2.25, 0.15, 5]),
      ...rail([0.25, 0.35, 3], [2.25, 0.35, 5]),
    ]);
    const result = constructionHeights(world, rails)!;
    expect(result.station.position).toEqual({ x: 0.25, y: 0.25, z: 3 });
    expect(result.station.metres).toBeCloseTo(9);
    expect(result.end.position.x).toBeCloseTo(2.25);
    expect(result.end.position.y).toBeCloseTo(0.25);
    expect(result.end.metres).toBeCloseTo(13);
  });

  it("has no markers without track", () => {
    expect(constructionHeights({} as WorldView, new Float32Array())).toBeNull();
  });
});

describe("height extrema", () => {
  function marker(index: number, elevationMetres: number): { position: { x: number; y: number; z: number }; metres: number } {
    return { position: { x: index, y: 0, z: elevationMetres / 4 }, metres: elevationMetres };
  }

  it("finds crests and valleys with at least three metres of change on both sides", () => {
    const samples = [0, 4, 8, 4, 0, 4, 9, 4, 0].map((elevation, index) => marker(index, elevation));

    expect(findHeightExtrema(samples)).toEqual([
      { marker: samples[2], kind: "crest" },
      { marker: samples[4], kind: "valley" },
      { marker: samples[6], kind: "crest" },
    ]);
  });

  it("filters small reversals without losing the larger crest", () => {
    const samples = [0, 4, 8, 7, 8.2, 7.5, 4, 0].map((elevation, index) => marker(index, elevation));

    expect(findHeightExtrema(samples)).toEqual([{ marker: samples[4], kind: "crest" }]);
  });

  it("does not label endpoints or height changes under three metres", () => {
    const samples = [0, 1, 2, 1, 0].map((elevation, index) => marker(index, elevation));

    expect(findHeightExtrema(samples)).toEqual([]);
  });
});
