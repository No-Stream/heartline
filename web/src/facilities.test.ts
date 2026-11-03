import { describe, expect, test } from "vitest";
import { buildFacilityGeometry, facilitySprites, type Facility } from "./facilities";

const facilities: Facility[] = [
  { id: 1, kind: "entrance", x: 3, y: 4, z: 2, rotation: 1, price: 0, ride_state: "idle", ride_progress: 0 },
  { id: 2, kind: "carousel", x: 5, y: 6, z: 0, rotation: 0, price: 5, ride_state: "idle", ride_progress: 0 },
  { id: 3, kind: "ferris_wheel", x: 7, y: 8, z: 0, rotation: 0, price: 5, ride_state: "idle", ride_progress: 0 },
];
const camera = { centerX: 5, centerY: 5, zoom: 1, rotation: 1 as const };
const viewport = { width: 800, height: 500 };
const style = { steel: "#123456", wood: "#654321", train: "#abcdef", ink: "#000000", lineWeight: 1 };

describe("facilities", () => {
  test("selects rotated building sprites relative to the camera", () => {
    expect(facilitySprites(facilities, 1)).toEqual([
      { id: "facility-1", sprite: "buildings/park_gate", x: 5, y: 4.5, height: 2, view: "rot0", heightDepth: true },
      { id: "facility-1", sprite: "buildings/ticket_booth", x: 3.5, y: 4.5, height: 2, view: "rot0", heightDepth: true, scale: 0.5 },
    ]);
  });
  test("uses actual cycle progress, keeping idle and unloading rides stationary", () => {
    const idle = buildFacilityGeometry(facilities, camera, viewport, style);
    const running = facilities.map((facility) => ({ ...facility, ride_state: "running" as const, ride_progress: 0.125 }));
    const moving = buildFacilityGeometry(running, camera, viewport, style);
    expect(buildFacilityGeometry(running, camera, viewport, style)).toEqual(moving);
    for (const id of ["facility-2", "facility-3"]) {
      expect(moving.filter((quad) => quad.id === id)).not.toEqual(idle.filter((quad) => quad.id === id));
    }
    const unloading = facilities.map((facility) => ({ ...facility, ride_state: "unloading" as const, ride_progress: 1 }));
    const unloaded = buildFacilityGeometry(unloading, camera, viewport, style);
    expect(buildFacilityGeometry(unloading, camera, viewport, style)).toEqual(unloaded);
  });
  test("builds distinct, pickable geometry for both flat rides", () => {
    const geometry = buildFacilityGeometry(facilities, camera, viewport, style);
    expect(geometry.filter((quad) => quad.id === "facility-2").length).toBeGreaterThan(4);
    expect(geometry.filter((quad) => quad.id === "facility-3").length).toBeGreaterThan(4);
    expect(new Set(geometry.map((quad) => quad.color.join(","))).size).toBeGreaterThan(1);
  });
});
