import { describe, expect, test } from "vitest";
import { terrainHeightAt, type WorldView } from "./scene";

describe("terrainHeightAt", () => {
  test("keeps a flat tile at its terrain height", () => {
    const world: WorldView = {
      dimensions: new Uint16Array([1, 1]),
      surfaces: new Int32Array([12, 12, 12, 12, 0, 0]),
      paths: new Int32Array(),
    };

    expect(terrainHeightAt(world, 0.35, 0.8)).toBe(12);
  });

  test("interpolates within the rendered triangles on both sides of the diagonal", () => {
    const world: WorldView = {
      dimensions: new Uint16Array([1, 1]),
      surfaces: new Int32Array([0, 4, 8, 2, 0, 0]),
      paths: new Int32Array(),
    };

    expect(terrainHeightAt(world, 0.75, 0.25)).toBe(4);
    expect(terrainHeightAt(world, 0.25, 0.75)).toBe(3);
    expect(terrainHeightAt(world, 0.5, 0.5)).toBe(4);
  });

  test("reads the terrain corners for the agent's tile", () => {
    const world: WorldView = {
      dimensions: new Uint16Array([2, 1]),
      surfaces: new Int32Array([0, 0, 0, 0, 0, 0, 8, 8, 8, 8, 0, 0]),
      paths: new Int32Array(),
    };

    expect(terrainHeightAt(world, 1.4, 0.6)).toBe(8);
  });
});
