import { describe, expect, it } from "vitest";
import { buildGhostQuads } from "./ghost";

const camera = { centerX: 0, centerY: 0, zoom: 1, rotation: 0 } as const;
const viewport = { width: 800, height: 500 };
const style = { steel: "#8899aa", wood: "#bb9988", train: "#aabbcc", ink: "#222222", lineWeight: 1 };
const rails = new Float32Array([0, 0,0,2, 0,1,0, 0,0,1, 1,0,2, 0,1,0, 0,0,1]);

describe("construction preview", () => {
  it("renders translucent green or red track without changing the geometry", () => {
    const valid = buildGhostQuads(rails, new Float32Array(), new Float32Array(), true, camera, viewport, style);
    const invalid = buildGhostQuads(rails, new Float32Array(), new Float32Array(), false, camera, viewport, style);
    expect(valid.length).toBeGreaterThan(0);
    expect(valid.map((quad) => quad.vertices)).toEqual(invalid.map((quad) => quad.vertices));
    expect(valid.every((quad) => quad.color[1] > quad.color[0] && quad.color[3] < 1)).toBe(true);
    expect(invalid.every((quad) => quad.color[0] > quad.color[1] && quad.color[3] < 1)).toBe(true);
  });
});
