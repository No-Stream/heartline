import { expect, it } from "vitest";
import { spatialise } from "./mapping";

it("spatialises points in camera screen space and attenuates distant sources", () => {
  const camera = { centerX: 0, centerY: 0, zoom: 2, rotation: 0 as const };
  const viewport = { width: 800, height: 600 };
  const centre = spatialise(camera, viewport, { x: 0, y: 0, z: 0 });
  const right = spatialise(camera, viewport, { x: 10, y: 0, z: 0 });
  expect(centre.pan).toBe(0);
  expect(centre.audibility).toBe(1);
  expect(right.pan).toBeGreaterThan(0);
  expect(right.audibility).toBeLessThan(centre.audibility);
  expect(right.pan).toBeLessThanOrEqual(0.8);
});
