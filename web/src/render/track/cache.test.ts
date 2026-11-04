import { expect, test } from "vitest";
import { createStaticTrackCache } from "./cache";
import { appendGeometry, clipGeometryAboveLevel, geometryIntersectsViewport, type Atlas, type GeometryQuad, type Scene } from "../scene";
import { buildTrackQuads, trackPaint, trackStyle } from "./geometry";

const frame = { width: 1, height: 1, anchor: { x: 0, y: 0 }, placements: { "1": { page: 0, x: 0, y: 0, width: 1, height: 1 } } };
const atlas: Atlas = { themeStyle: { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 }, pages: { "1": [{ file: "test.png", width: 1, height: 1 }] }, frames: { "ground/grass_light_0/any/static/0": frame } };

test("static track cache invalidates for changed camera, viewport, geometry or colours", () => {
  const cache = createStaticTrackCache(atlas);
  const rails = new Float32Array([1, 1, 2, 1, 0, 1, 0, 0, 0, 1, 3, 2, 1, 0, 1, 0, 0, 0, 1]);
  const ties = new Float32Array();
  const supports = new Float32Array();
  const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
  const viewport = { width: 400, height: 300 };
  const colors = { track: "coral", supports: "timber", trains: "sage" } as const;
  const first = cache(rails, ties, supports, colors, camera, viewport);
  expect(first.length).toBeGreaterThan(0);
  expect(cache(rails, ties, supports, colors, { ...camera }, { ...viewport })).toBe(first);
  expect(cache(rails, ties, supports, colors, { ...camera, rotation: 1 }, viewport)).not.toBe(first);
  const second = cache(rails, ties, supports, colors, camera, viewport);
  expect(cache(rails, ties, new Float32Array(), colors, camera, viewport)).not.toBe(second);
  const third = cache(rails, ties, supports, colors, camera, viewport);
  expect(cache(rails, ties, supports, colors, camera, { ...viewport, width: 401 })).not.toBe(third);
  const fourth = cache(rails, ties, supports, colors, camera, viewport);
  expect(cache(rails, ties, supports, { ...colors, track: "teal" }, camera, viewport)).not.toBe(fourth);
});

test("caches only viewport-visible track while preserving rendered triangles and construction IDs", () => {
  const cache = createStaticTrackCache(atlas);
  const rails = new Float32Array([
    1, 1, 2, 1, 0, 1, 0, 0, 0, 1, 3, 2, 1, 0, 1, 0, 0, 0, 1,
    1, 100, 100, 1, 0, 1, 0, 0, 0, 1, 102, 100, 1, 0, 1, 0, 0, 0, 1,
  ]);
  const empty = new Float32Array();
  const viewport = { width: 400, height: 300 };
  for (const rotation of [0, 1, 2, 3] as const) {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation };
    const colors = { track: "coral", supports: "timber", trains: "sage" } as const;
    const all = buildTrackQuads(rails, empty, empty, empty, camera, viewport, trackPaint(trackStyle(atlas), colors));
    const visible = cache(rails, empty, empty, colors, camera, viewport);
    expect(visible.length).toBeGreaterThan(0);
    expect(visible.length).toBeLessThan(all.length);
    for (const underground of [false, true]) {
      const prepare = (quads: readonly GeometryQuad[]) => {
        const scene: Scene = { triangles: [] };
        const construction = quads.map((quad) => ({ ...quad, id: `construction-${quad.id}` }));
        appendGeometry(scene, underground ? clipGeometryAboveLevel(construction, 4, 4) : construction, viewport);
        return scene;
      };
      expect(prepare(visible)).toEqual(prepare(all));
    }
  }
});

test("retains quads touching the viewport boundary", () => {
  const viewport = { width: 100, height: 100 };
  const vertices = (xs: number[], ys: number[]) => xs.map((x, index) => ({ x, y: ys[index], depth: 0, height: 0, u: 0, v: 0 })) as GeometryQuad["vertices"];
  const quad = (xs: number[], ys: number[]): GeometryQuad => ({ vertices: vertices(xs, ys), color: [1, 1, 1, 1], kind: "track", id: "edge", tileX: 0, tileY: 0 });
  for (const shape of [
    quad([-2, 0, 0, -2], [20, 20, 30, 30]),
    quad([100, 102, 102, 100], [20, 20, 30, 30]),
    quad([20, 30, 30, 20], [-2, -2, 0, 0]),
    quad([20, 30, 30, 20], [100, 100, 102, 102]),
  ]) expect(geometryIntersectsViewport(shape, viewport)).toBe(true);
  expect(geometryIntersectsViewport(quad([-2, -1, -1, -2], [20, 20, 30, 30]), viewport)).toBe(false);
});
