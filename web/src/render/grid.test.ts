import { describe, expect, it } from "vitest";
import { buildGridQuads } from "./grid";
import type { Triangle, Vertex } from "./scene";

function tile(x: number, y: number): [Triangle, Triangle] {
  const vertex = (pointX: number, pointY: number): Vertex => ({ x: pointX * 20, y: pointY * 20, depth: 0, height: 0, u: 0, v: 0 });
  const corners = [vertex(x, y), vertex(x + 1, y), vertex(x + 1, y + 1), vertex(x, y + 1)];
  const triangle = (vertices: [Vertex, Vertex, Vertex]): Triangle => ({ vertices, page: 0, color: [1, 1, 1, 1], kind: "terrain", tileX: x, tileY: y, height: 0 });
  return [triangle([corners[0], corners[1], corners[2]]), triangle([corners[0], corners[2], corners[3]])];
}

describe("build grid overlay", () => {
  it("draws each tile perimeter once without drawing the diagonal", () => {
    const quads = buildGridQuads([...tile(0, 0), ...tile(1, 0)], [0.1, 0.1, 0.2, 0.2]);
    expect(quads).toHaveLength(7);
    expect(quads.every((quad) => quad.color[3] === 0.2)).toBe(true);
    expect(quads.some((quad) => quad.vertices[0].x === 0 && quad.vertices[0].y === 0 && quad.vertices[1].x === 20 && quad.vertices[1].y === 20)).toBe(false);
  });

  it("ignores standing geometry", () => {
    const [first, second] = tile(0, 0);
    expect(buildGridQuads([{ ...first, kind: "building" }, { ...second, kind: "building" }], [0, 0, 0, 0.2])).toEqual([]);
  });
});
