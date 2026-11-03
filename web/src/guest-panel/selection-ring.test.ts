import { describe, expect, it } from "vitest";
import { buildSelectionRing } from "./selection-ring";
import type { Atlas, WorldSprite } from "../render/scene";

const camera = { centerX: 10, centerY: 12, zoom: 1, rotation: 0 } as const;
const viewport = { width: 640, height: 480 };
const atlas = { themeStyle: { steel: "#000000", wood: "#000000", train: "#000000", ink: "#302947", lineWeight: 1 } } as Atlas;
const guest: WorldSprite = { id: "guest-10", sprite: "people/maya", x: 10, y: 12, height: 4 };

describe("buildSelectionRing", () => {
  it("builds a fine ring at the selected guest's feet in the theme ink", () => {
    const ring = buildSelectionRing(guest, camera, viewport, atlas);

    expect(ring).toHaveLength(24);
    expect(ring.every((quad) => quad.id === "selection-guest-10" && quad.kind === "selection-ring")).toBe(true);
    expect(ring.every((quad) => quad.color[3] === 0.84)).toBe(true);
    expect(ring[0].color).toEqual([0x30 / 255, 0x29 / 255, 0x47 / 255, 0.84]);
    const vertexHeights = ring.flatMap((quad) => quad.vertices.map((vertex) => vertex.height));
    expect(new Set(vertexHeights)).toEqual(new Set([guest.height]));
    const screenX = ring.flatMap((quad) => quad.vertices.map((vertex) => vertex.x));
    const screenY = ring.flatMap((quad) => quad.vertices.map((vertex) => vertex.y));
    expect((Math.min(...screenX) + Math.max(...screenX)) / 2).toBeCloseTo(320);
    expect((Math.min(...screenY) + Math.max(...screenY)) / 2).toBeCloseTo(207.52);
  });

  it("returns no geometry when no guest is selected", () => {
    expect(buildSelectionRing(undefined, camera, viewport, atlas)).toEqual([]);
  });

  it("requires the atlas ink style", () => {
    expect(() => buildSelectionRing(guest, camera, viewport, {} as Atlas)).toThrow("Atlas is missing selection ink style");
  });
});
