import { expect, test } from "vitest";
import { createStaticTrackCache } from "./cache";
import type { Atlas } from "../scene";

const frame = { width: 1, height: 1, anchor: { x: 0, y: 0 }, placements: { "1": { page: 0, x: 0, y: 0, width: 1, height: 1 } }, properties: { "coaster-steel": "#ff0000", "coaster-wood": "#aa6600", "coaster-train": "#00ff00", "coaster-ink": "#221133", "coaster-line-weight": "1" } };
const atlas: Atlas = { pages: { "1": [{ file: "test.png", width: 1, height: 1 }] }, frames: { "ground/grass_light_0/any/static/0": frame } };

test("static track cache invalidates for changed camera, viewport or geometry", () => {
  const cache = createStaticTrackCache(atlas);
  const rails = new Float32Array([1, 1, 2, 1, 0, 1, 0, 0, 0, 1, 3, 2, 1, 0, 1, 0, 0, 0, 1]);
  const ties = new Float32Array();
  const supports = new Float32Array();
  const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
  const viewport = { width: 400, height: 300 };
  const first = cache(rails, ties, supports, camera, viewport);
  expect(first.length).toBeGreaterThan(0);
  expect(cache(rails, ties, supports, { ...camera }, { ...viewport })).toBe(first);
  expect(cache(rails, ties, supports, { ...camera, rotation: 1 }, viewport)).not.toBe(first);
  const second = cache(rails, ties, supports, camera, viewport);
  expect(cache(rails, ties, new Float32Array(), camera, viewport)).not.toBe(second);
  const third = cache(rails, ties, supports, camera, viewport);
  expect(cache(rails, ties, supports, camera, { ...viewport, width: 401 })).not.toBe(third);
});
