import { describe, expect, it } from "vitest";
import { buildLandscapingGhostQuads, createLandscapingTool, type LandscapingPreview, type TerrainEditCommand, type LandscapingSelection } from "./landscaping-tool";
import type { WorldView } from "./render/scene";
import type { Camera, Viewport } from "./camera";

const selection: LandscapingSelection = {
  tool: "raise_tiles",
  size: 3,
  height: 12,
  bottom: 4,
  top: 8,
};

function setup() {
  const commands: TerrainEditCommand[] = [];
  let changes = 0;
  const tool = createLandscapingTool((command) => commands.push(command), () => { changes += 1; });
  return { tool, commands, changes: () => changes };
}

describe("landscaping tool", () => {
  it("previews a centered brush and applies its area from the normalized top-left", () => {
    const { tool, commands } = setup();
    tool.activate(selection);
    tool.handle({ type: "hover", tile: { x: 8, y: 5 } });
    expect(tool.state().preview).toEqual({ x: 7, y: 4, size: 3, tool: "raise_tiles" });
    tool.handle({ type: "select", tile: { x: 8, y: 5 } });
    expect(commands).toEqual([{
      type: "terrain-edit", command: "raise_tiles", x: 7, y: 4, size: 3,
      corner: 0, steps: 1, height: 12, bottom: 4, top: 8,
    }]);
  });

  it("uses the selected corner for a shared-vertex edit", () => {
    const { tool, commands } = setup();
    tool.activate({ ...selection, tool: "lower_corner", size: 9 });
    tool.handle({ type: "select", tile: { x: 3, y: 6, corner: "south_east" } });
    expect(commands[0]).toMatchObject({ command: "lower_corner", x: 3, y: 6, corner: 2, size: 1, steps: 1 });
  });

  it("applies a drag once per visited tile and ignores repeated pointer samples", () => {
    const { tool, commands } = setup();
    tool.activate(selection);
    tool.handle({ type: "drag-start", tile: { x: 2, y: 2 } });
    tool.handle({ type: "drag-move", tile: { x: 3, y: 2 } });
    tool.handle({ type: "drag-move", tile: { x: 3, y: 2 } });
    tool.handle({ type: "drag-end", tile: { x: 3, y: 2 } });
    expect(commands.map(({ x, y }) => [x, y])).toEqual([[1, 1], [2, 1]]);
  });

  it("fills skipped tiles when a pointer drag crosses several tiles between samples", () => {
    const { tool, commands } = setup();
    tool.activate({ ...selection, size: 1 });
    tool.handle({ type: "drag-start", tile: { x: 1, y: 1 } });
    tool.handle({ type: "drag-move", tile: { x: 5, y: 1 } });
    tool.handle({ type: "drag-end", tile: { x: 5, y: 1 } });
    expect(commands.map(({ x, y }) => [x, y])).toEqual([[1, 1], [2, 1], [3, 1], [4, 1], [5, 1]]);
  });

  it("discards a cancelled drag and ignores input when inactive", () => {
    const { tool, commands } = setup();
    expect(tool.handle({ type: "select", tile: { x: 1, y: 1 } })).toBe(false);
    tool.activate(selection);
    tool.handle({ type: "drag-start", tile: { x: 1, y: 1 } });
    tool.handle({ type: "drag-cancel" });
    tool.handle({ type: "drag-end", tile: { x: 2, y: 1 } });
    expect(commands).toEqual([]);
  });

  it("validates tool settings and clears the preview when cancelled", () => {
    const { tool, commands, changes } = setup();
    expect(() => tool.activate({ ...selection, size: 10 })).toThrow();
    expect(() => tool.activate({ ...selection, height: 1.5 })).toThrow();
    tool.activate(selection);
    tool.handle({ type: "hover", tile: { x: 1, y: 1 } });
    tool.cancel();
    expect(tool.state()).toMatchObject({ active: false, preview: null });
    expect(commands).toEqual([]);
    expect(changes()).toBeGreaterThan(0);
  });

  it("keeps edge brushes inside the world and refuses brushes larger than the map", () => {
    const { tool, commands } = setup();
    tool.setDimensions(5, 5);
    tool.activate(selection);
    tool.handle({ type: "select", tile: { x: 0, y: 0 } });
    expect(commands[0]).toMatchObject({ x: 0, y: 0, size: 3 });
    expect(() => tool.activate({ ...selection, size: 7 })).toThrow();
  });

  it("builds a preview footprint from terrain heights or the selected underground level", () => {
    const world: WorldView = {
      dimensions: new Uint16Array([2, 2]),
      surfaces: new Int32Array(Array.from({ length: 4 }, () => [8, 8, 8, 8, 0, 0]).flat()),
      paths: new Int32Array(),
    };
    const camera: Camera = { centerX: 1, centerY: 1, zoom: 1, rotation: 0 };
    const viewport: Viewport = { width: 800, height: 500 };
    const preview: LandscapingPreview = { x: 0, y: 0, size: 1, tool: "carve" };
    const surface = buildLandscapingGhostQuads(preview, world, camera, viewport);
    expect(surface).toHaveLength(1);
    expect(surface[0].vertices.map(({ height }) => height)).toEqual([8, 8, 8, 8]);
    const underground = buildLandscapingGhostQuads({ ...preview, height: 3 }, world, camera, viewport);
    expect(underground[0].vertices.map(({ height }) => height)).toEqual([3, 3, 3, 3]);
    expect(() => buildLandscapingGhostQuads({ ...preview, x: 2 }, world, camera, viewport)).toThrow();
  });

  it("shows the terrain refusal message returned by the simulation", () => {
    const { tool, changes } = setup();
    tool.activate(selection);
    tool.receive({ type: "terrain-result", success: false, message: "A ride occupies this tile.", invalidatedPaths: new Int32Array() });
    expect(tool.state().message).toBe("A ride occupies this tile.");
    expect(changes()).toBeGreaterThan(0);
  });
});
