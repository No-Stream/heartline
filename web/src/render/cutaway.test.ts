import { describe, expect, test } from "vitest";
import { CUTAWAY_TRACK_CLEARANCE, CutSide, cutSideVisibleAt, cutawayLevelForTrack, polygonTouchesCutaway, type Cutaway } from "./cutaway";
import type { Vertex } from "./scene";

const cutaway: Cutaway = { x: 100, y: 80, radius: 20, level: 6 };
const vertex = (x: number, y: number): Vertex => ({ x, y, depth: 0, height: 0, u: 0, v: 0 });

describe("cutaway window", () => {
  test("surface geometry shows outside the window and cut geometry inside it", () => {
    expect(cutSideVisibleAt(CutSide.outside, cutaway, 100, 80)).toBe(false);
    expect(cutSideVisibleAt(CutSide.inside, cutaway, 100, 80)).toBe(true);
    expect(cutSideVisibleAt(CutSide.outside, cutaway, 100, 101)).toBe(true);
    expect(cutSideVisibleAt(CutSide.inside, cutaway, 100, 101)).toBe(false);
    expect(cutSideVisibleAt(CutSide.everywhere, cutaway, 100, 80)).toBe(true);
    expect(cutSideVisibleAt(CutSide.inside, undefined, 0, 0)).toBe(true);
  });

  test("finds polygons that reach into the window, including ones larger than it", () => {
    expect(polygonTouchesCutaway([vertex(0, 0), vertex(300, 0), vertex(300, 300)], cutaway)).toBe(true);
    expect(polygonTouchesCutaway([vertex(115, 60), vertex(130, 60), vertex(130, 70)], cutaway)).toBe(true);
    expect(polygonTouchesCutaway([vertex(118, 62), vertex(130, 62), vertex(130, 50)], cutaway)).toBe(false);
    expect(polygonTouchesCutaway([vertex(0, 0), vertex(10, 0), vertex(10, 10)], cutaway)).toBe(false);
  });

  test("cuts just above the track end", () => {
    expect(cutawayLevelForTrack(1.3)).toBe(5 + CUTAWAY_TRACK_CLEARANCE);
    expect(cutawayLevelForTrack(-0.5)).toBe(-2 + CUTAWAY_TRACK_CLEARANCE);
    expect(() => cutawayLevelForTrack(Number.NaN)).toThrow(RangeError);
  });
});
