import { describe, expect, it } from "vitest";
import { constructionCutawayTarget, landscapingCutawayTarget, parseStoredUndergroundCutawayMode, pathCutawayTarget, shouldShowAutomaticCutaway } from "./underground-cutaway";
import type { WorldView } from "./render/scene";

function flatWorld(height: number): WorldView {
  return {
    dimensions: new Uint16Array([3, 3]),
    surfaces: new Int32Array(Array.from({ length: 3 * 3 }, () => [height, height, height, height, 0, 0]).flat()),
    paths: new Int32Array(),
  };
}

describe("automatic underground cutaway", () => {
  it("accepts only auto, off, and on as persisted modes", () => {
    expect(parseStoredUndergroundCutawayMode("auto")).toBe("auto");
    expect(parseStoredUndergroundCutawayMode("off")).toBe("off");
    expect(parseStoredUndergroundCutawayMode("on")).toBe("on");
    expect(parseStoredUndergroundCutawayMode("invalid")).toBeUndefined();
    expect(parseStoredUndergroundCutawayMode(null)).toBeUndefined();
  });

  it("shows underground targets automatically, always shows them when on, and hides them when off", () => {
    const undergroundTarget = { x: 1, y: 1, z: 1, level: 7, belowTerrain: true };
    const surfaceTarget = { ...undergroundTarget, belowTerrain: false };
    expect(shouldShowAutomaticCutaway("auto", undergroundTarget)).toBe(true);
    expect(shouldShowAutomaticCutaway("auto", surfaceTarget)).toBe(false);
    expect(shouldShowAutomaticCutaway("on", surfaceTarget)).toBe(true);
    expect(shouldShowAutomaticCutaway("off", undergroundTarget)).toBe(false);
    expect(shouldShowAutomaticCutaway("on", null)).toBe(false);
  });

  it("uses the placed coaster end height to decide whether the track is underground", () => {
    expect(constructionCutawayTarget({ position: { x: 2, y: 1, z: 1.25 }, metres: -1 }))
      .toMatchObject({ x: 2, y: 1, z: 1.25, level: 8, belowTerrain: true });
    expect(constructionCutawayTarget({ position: { x: 2, y: 1, z: 1.25 }, metres: 0 })?.belowTerrain).toBe(false);
    expect(constructionCutawayTarget(null)).toBeNull();
  });

  it("follows the underground endpoint of a path preview and ignores a surface endpoint", () => {
    const underground = pathCutawayTarget(flatWorld(20), Int32Array.of(1, 1, 4, 0, 0, 0, 0, 0, 0));
    expect(underground).toMatchObject({ x: 1.5, y: 1.5, z: 1, level: 7, belowTerrain: true });

    const surface = pathCutawayTarget(flatWorld(4), Int32Array.of(1, 1, 4, 0, 0, 0, 0, 0, 0));
    expect(surface?.belowTerrain).toBe(false);
  });

  it("follows a carve preview when its target floor lies below terrain", () => {
    const target = landscapingCutawayTarget(
      flatWorld(12),
      { x: 0, y: 0, size: 2, tool: "carve" },
      { tool: "carve", size: 2, height: 8, bottom: 4, top: 10 },
    );
    expect(target).toMatchObject({ x: 1, y: 1, z: 1, level: 7, belowTerrain: true });
  });
});
