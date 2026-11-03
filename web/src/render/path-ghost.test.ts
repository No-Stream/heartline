import { describe, expect, test } from "vitest";
import { projectWorld } from "../camera";
import { buildPathGhostQuads, decodePathPreview } from "./path-ghost";

const camera = { centerX: 0, centerY: 0, zoom: 1, rotation: 0 } as const;
const viewport = { width: 800, height: 500 };

describe("path preview", () => {
  test("projects the path slope and marks valid and invalid placements", () => {
    const path = { x: 1, y: 2, z: 4, slope: 1 };
    const [valid, invalid] = buildPathGhostQuads([{ ...path, refusal: null }, { ...path, refusal: "Too steep" }], camera, viewport);
    expect(valid.vertices.map(({ x, y }) => ({ x, y }))).toEqual([
      projectWorld(camera, viewport, { x: 1, y: 2, z: 1.25 }),
      projectWorld(camera, viewport, { x: 2, y: 2, z: 1.25 }),
      projectWorld(camera, viewport, { x: 2, y: 3, z: 1 }),
      projectWorld(camera, viewport, { x: 1, y: 3, z: 1 }),
    ]);
    expect(valid.vertices).toEqual(invalid.vertices);
    expect(valid.color[1]).toBeGreaterThan(valid.color[0]);
    expect(invalid.color[0]).toBeGreaterThan(invalid.color[1]);
    expect(valid.color[3]).toBeLessThan(1);
  });

  test("decodes each tile's refusal and rejects malformed preview data", () => {
    const preview = decodePathPreview(new Int32Array([1, 2, 4, 1, 0, 0, 0, 2, 2, 4, 0, 0, 0, 4]));
    expect(preview[0].refusal).toBeNull();
    expect(preview[1].refusal).toMatch(/connect/);
    expect(() => decodePathPreview(new Int32Array([1, 2]))).toThrow("Invalid path preview");
    expect(() => decodePathPreview(new Int32Array([1, 2, 3, 0, 0, 0, 99]))).toThrow("Invalid path preview refusal");
  });
});
