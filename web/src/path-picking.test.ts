import { describe, expect, it } from "vitest";
import { projectWorld } from "./camera";
import { pathTileFromPick } from "./path-picking";
import { buildScene, pickScene, type Atlas, type Pick, type WorldView } from "./render/scene";

const frame = { width: 4, height: 4, anchor: { x: 2, y: 2 }, placements: { "1": { page: 0, x: 0, y: 0, width: 4, height: 4 } } };
const atlas: Atlas = {
  pages: { "1": [{ file: "page.png", width: 4, height: 4 }] },
  frames: {
    "ground/grass_light_0/any/static/0": frame,
    "ground/path_0000/any/static/0": frame,
    ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
      [`ground/cliff_${side}_${variant}/any/static/0`, frame],
      [`ground/cliff_step_${side}_${variant}/any/static/0`, frame],
    ]))),
  },
};
const camera = { centerX: 1, centerY: 1, zoom: 1, rotation: 0 as const };
const viewport = { width: 400, height: 300 };

function world(paths: number[] = []): WorldView {
  return {
    dimensions: new Uint16Array([2, 1]),
    surfaces: new Int32Array([
      4, 5, 5, 4, 0, 0,
      0, 0, 0, 0, 0, 0,
    ]),
    paths: new Int32Array(paths),
  };
}

function pick(kind: "terrain" | "path", x: number, height: number): Pick {
  return { kind, x, y: 0, height };
}

describe("path placement picks", () => {
  it("uses the lowest terrain corner as the route endpoint height", () => {
    expect(pathTileFromPick(pick("terrain", 0, 4.8), world())).toEqual({ x: 0, y: 0, z: 4, kind: "terrain" });
  });

  it("resolves flat and sloped path picks to their stored integer base height", () => {
    const flat = world([0, 0, 4, 0]);
    expect(pathTileFromPick(pick("path", 0, 4), flat)).toEqual({ x: 0, y: 0, z: 4, kind: "path" });

    const sloped = world([0, 0, 8, 2]);
    expect(pathTileFromPick(pick("path", 0, 8.25), sloped)).toEqual({ x: 0, y: 0, z: 8, kind: "path" });
    expect(pathTileFromPick(pick("path", 0, 8.75), sloped)).toEqual({ x: 0, y: 0, z: 8, kind: "path" });
  });

  it("resolves the interpolated height returned by the actual renderer picker", () => {
    const sloped = { ...world([0, 0, 8, 2]), surfaces: new Int32Array(12) };
    const scene = buildScene(sloped, camera, viewport, atlas);
    const screen = projectWorld(camera, viewport, { x: 0.75, y: 0.5, z: 8.75 / 4 });
    const renderedPick = pickScene(scene, screen.x, screen.y);

    expect(renderedPick?.kind).toBe("path");
    expect(pathTileFromPick(renderedPick, sloped)).toEqual({ x: 0, y: 0, z: 8, kind: "path" });
  });

  it("resolves a renderer pick on queue railing geometry", () => {
    const queue = { ...world([0, 0, 8, 2]), surfaces: new Int32Array(12), pathMetadata: new Int32Array([1, 0]) };
    const scene = buildScene(queue, camera, viewport, atlas);
    const railing = scene.triangles.find((triangle) => triangle.kind === "path-railing");
    expect(railing).toBeDefined();
    const screen = {
      x: railing!.vertices.reduce((total, vertex) => total + vertex.x, 0) / railing!.vertices.length,
      y: railing!.vertices.reduce((total, vertex) => total + vertex.y, 0) / railing!.vertices.length,
    };
    const renderedPick = pickScene(scene, screen.x, screen.y);

    expect(renderedPick).toMatchObject({ kind: "path", x: 0, y: 0 });
    expect(pathTileFromPick(renderedPick, queue)).toEqual({ x: 0, y: 0, z: 8, kind: "path" });
  });

  it("selects the matching layer and declines picks on an ambiguous shared edge", () => {
    const stacked = world([0, 0, 4, 0, 0, 0, 6, 0]);
    expect(pathTileFromPick(pick("path", 0, 6), stacked)).toEqual({ x: 0, y: 0, z: 6, kind: "path" });

    const touchingLayers = world([0, 0, 4, 1, 0, 0, 5, 2]);
    expect(pathTileFromPick(pick("path", 0, 5), touchingLayers)).toBeNull();
  });

  it("declines non-path picks and validates snapshots and coordinates", () => {
    expect(pathTileFromPick({ kind: "water", x: 0, y: 0, height: 4 }, world())).toBeNull();
    expect(pathTileFromPick(null, world())).toBeNull();
    expect(() => pathTileFromPick(pick("terrain", 2, 0), world())).toThrow(RangeError);
    expect(() => pathTileFromPick(pick("path", 0, 4), { ...world(), paths: new Int32Array([0, 0, 4]) })).toThrow("Invalid world snapshot");
  });
});
