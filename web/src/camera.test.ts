import { describe, expect, it } from "vitest";
import { applyCameraAction, projectWorld, unprojectGround, type Camera } from "./camera";
import { guestViewForRotation } from "./guest-view";

const viewport = { width: 800, height: 600 };
const camera: Camera = { centerX: 12, centerY: 20, zoom: 1, rotation: 0 };

describe("isometric camera", () => {
  it("projects and unprojects positions at known elevations in every rotation", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      const view = { ...camera, rotation };
      const world = { x: 15.25, y: 17.75, z: 1.5 };
      const result = unprojectGround(view, viewport, projectWorld(view, viewport, world), world.z);
      expect(result.x).toBeCloseTo(world.x);
      expect(result.y).toBeCloseTo(world.y);
      expect(result.z).toBe(world.z);
    }
  });

  it("keeps the camera center fixed during rotation", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      expect(projectWorld({ ...camera, rotation }, viewport, { x: 12, y: 20, z: 0 }))
        .toEqual({ x: 400, y: 300 });
    }
  });

  it("pans in screen coordinates for every view", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      const view = { ...camera, rotation };
      const point = { x: 15, y: 23, z: 0 };
      const before = projectWorld(view, viewport, point);
      const after = projectWorld(applyCameraAction(view, viewport, { type: "pan", dx: 20, dy: -10 }), viewport, point);
      expect(after.x).toBeCloseTo(before.x + 20);
      expect(after.y).toBeCloseTo(before.y - 10);
    }
  });

  it("zooms smoothly around the chosen screen position and clamps the scale", () => {
    const anchor = { x: 190, y: 240 };
    const world = unprojectGround(camera, viewport, anchor);
    const zoomed = applyCameraAction(camera, viewport, { type: "zoom", factor: 1.17, anchor });
    expect(zoomed.zoom).toBeCloseTo(1.17);
    expect(projectWorld(zoomed, viewport, world).x).toBeCloseTo(anchor.x);
    expect(projectWorld(zoomed, viewport, world).y).toBeCloseTo(anchor.y);
    expect(applyCameraAction(camera, viewport, { type: "zoom", factor: 100, anchor }).zoom).toBe(4);
    expect(applyCameraAction(camera, viewport, { type: "zoom", factor: 0.001, anchor }).zoom).toBe(0.5);
  });

  it("wraps rotations in either direction", () => {
    expect(applyCameraAction(camera, viewport, { type: "rotate", turns: -1 }).rotation).toBe(3);
    expect(applyCameraAction({ ...camera, rotation: 3 }, viewport, { type: "rotate", turns: 1 }).rotation).toBe(0);
  });
});

it("keeps a world-facing guest oriented consistently as the camera rotates", () => {
  expect(([0, 1, 2, 3] as const).map((rotation) => guestViewForRotation(rotation)))
    .toEqual(["se", "sw", "nw", "ne"]);
});
