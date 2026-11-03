import { describe, expect, it } from "vitest";
import { projectWorld, type Camera } from "./camera";
import { followTrackEnd, railPoints } from "./construction-camera";

const viewport = { width: 800, height: 500 };
const camera: Camera = { centerX: 30, centerY: 30, zoom: 1, rotation: 0 };

function rails(points: [number, number, number][]): Float32Array {
  return new Float32Array(points.slice(1).flatMap((end, index) => [0, ...points[index], 0, 1, 0, 0, 0, 1, ...end, 0, 1, 0, 0, 0, 1]));
}

describe("camera follow of the open track end", () => {
  it("reads the start and end of every rail segment", () => {
    expect(railPoints(rails([[1, 2, 3], [4, 5, 6], [7, 8, 9]]))).toEqual([{ x: 1, y: 2, z: 3 }, { x: 4, y: 5, z: 6 }, { x: 4, y: 5, z: 6 }, { x: 7, y: 8, z: 9 }]);
  });

  it("leaves the camera alone while the preview is comfortably in view", () => {
    const points = [{ x: 30, y: 30, z: 0 }, { x: 32, y: 30, z: 0 }];
    expect(followTrackEnd(camera, viewport, points, 1)).toEqual({ camera, settled: true });
  });

  it("eases towards bringing an off-screen preview just inside the comfortable area, and snaps with a fraction of one", () => {
    const points = [{ x: 50, y: 50, z: 2 }, { x: 52, y: 50, z: 2 }];
    const eased = followTrackEnd(camera, viewport, points, 0.25);
    const snapped = followTrackEnd(camera, viewport, points, 1);
    expect(eased.settled).toBe(false);
    const distance = (view: Camera) => Math.hypot(projectWorld(view, viewport, points[0]).y - 250, projectWorld(view, viewport, points[0]).x - 400);
    expect(distance(eased.camera)).toBeLessThan(distance(camera));
    expect(distance(snapped.camera)).toBeLessThan(distance(eased.camera));
    for (const point of points) {
      const screen = projectWorld(snapped.camera, viewport, point);
      expect(screen.x).toBeGreaterThanOrEqual(800 * 0.2 - 1e-6);
      expect(screen.x).toBeLessThanOrEqual(800 * 0.8 + 1e-6);
      expect(screen.y).toBeGreaterThanOrEqual(500 * 0.2 - 1e-6);
      expect(screen.y).toBeLessThanOrEqual(500 * 0.8 + 1e-6);
    }
    expect(followTrackEnd(snapped.camera, viewport, points, 1).settled).toBe(true);
  });
});
