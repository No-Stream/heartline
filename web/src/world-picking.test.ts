import { projectWorld, type Camera, type Viewport } from "./camera";
import { describe, expect, it } from "vitest";
import { blockBoundaryIndexFromPick, entityIdFromPick, facilityIdFromPick, landscapingTileFromPick, sceneryTileFromPick } from "./world-picking";
import type { Pick, WorldView } from "./render/scene";

const camera: Camera = { centerX: 1, centerY: 1, zoom: 1, rotation: 0 };
const viewport: Viewport = { width: 400, height: 300 };

function world(): WorldView {
  return {
    dimensions: new Uint16Array([2, 1]),
    surfaces: new Int32Array([0, 4, 8, 12, 0, 0, 0, 0, 0, 0, 0, 0]),
    paths: new Int32Array(),
  };
}

function surfacePick(kind: "terrain" | "path" | "carved-floor", x = 0, height = 8): Pick {
  return { kind, x, y: 0, height };
}

describe("world picking helpers", () => {
  it("chooses the closest corner using the terrain's individual corner heights", () => {
    const pick = surfacePick("terrain");
    const target = projectWorld(camera, viewport, { x: 1, y: 1, z: 12 / 4 });

    expect(landscapingTileFromPick(pick, world(), target, camera, viewport)).toEqual({
      x: 0,
      y: 0,
      corner: "south_east",
    });
  });

  it("uses picked height for paths and carved floors and includes it underground", () => {
    const target = projectWorld(camera, viewport, { x: 1, y: 0, z: 8 / 4 });
    expect(landscapingTileFromPick(surfacePick("path"), world(), target, camera, viewport))
      .toEqual({ x: 0, y: 0, corner: "north_east" });
    expect(landscapingTileFromPick(surfacePick("carved-floor", 0, 7.6), world(), target, camera, viewport, 2))
      .toEqual({ x: 0, y: 0, height: 8, corner: "north_east" });
    expect(landscapingTileFromPick(surfacePick("terrain"), undefined, target, camera, viewport)).toBeNull();
    expect(landscapingTileFromPick({ kind: "water", x: 0, y: 0, height: 8 }, world(), target, camera, viewport)).toBeNull();
  });

  it("maps terrain quarter centers and declines other surface kinds", () => {
    const pick = surfacePick("terrain", 1, 8);
    const target = projectWorld(camera, viewport, { x: 1.75, y: 0.25, z: 2 });
    expect(sceneryTileFromPick(pick, target, camera, viewport)).toEqual({ x: 1, y: 0, z: 8, quarter: 1 });
    expect(sceneryTileFromPick(surfacePick("path"), target, camera, viewport)).toBeNull();
    expect(sceneryTileFromPick(pick, null, camera, viewport)).toBeNull();
  });

  it("extracts renderer entity IDs and parses numeric facility and block IDs", () => {
    expect(entityIdFromPick({ kind: "scenery", id: "scenery-1:2:8:3", x: 0, y: 0, height: 0 }, "scenery"))
      .toBe("1:2:8:3");
    expect(entityIdFromPick(surfacePick("terrain"), "scenery")).toBeUndefined();
    expect(entityIdFromPick({ kind: "scenery", id: "scenery-", x: 0, y: 0, height: 0 }, "scenery")).toBeUndefined();
    expect(facilityIdFromPick({ kind: "facility", id: "facility-23", x: 0, y: 0, height: 0 })).toBe(23);
    expect(facilityIdFromPick({ kind: "facility", id: "facility-invalid", x: 0, y: 0, height: 0 })).toBeUndefined();
    expect(blockBoundaryIndexFromPick({ kind: "block-boundary", id: "block-boundary-4", x: 0, y: 0, height: 0 })).toBe(4);
    expect(blockBoundaryIndexFromPick(surfacePick("terrain"))).toBeNull();
  });
});
