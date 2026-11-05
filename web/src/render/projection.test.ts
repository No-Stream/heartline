import { describe, expect, test } from "vitest";
import { projectVertex } from "./projection";

const viewport = { width: 400, height: 300 };
const point = { x: 3, y: 5, z: 2 };

describe("projectVertex", () => {
  test.each([
    [0, 168, 166, 10],
    [1, 40, 70, 0],
    [2, 232, 6, -6],
    [3, 360, 102, 4],
  ] as const)("projects screen coordinates and world depth at rotation %i", (rotation, x, y, depth) => {
    const camera = { centerX: 1, centerY: 2, zoom: 1, rotation };
    expect(projectVertex(camera, viewport, point)).toEqual({ x, y, depth, height: 8, u: 0, v: 0 });
    expect(projectVertex({ ...camera, centerX: 9, centerY: -2 }, viewport, point).depth).toBe(depth);
    expect(projectVertex(camera, viewport, point, 0.25).depth).toBe(depth + 0.25);
  });
});
