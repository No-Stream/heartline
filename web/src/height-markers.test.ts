import { describe, expect, it } from "vitest";
import { constructionHeights } from "./height-markers";
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
