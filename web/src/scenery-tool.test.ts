import { describe, expect, it } from "vitest";
import { createSceneryTool, scenerySprites, type SceneryCommand } from "./scenery-tool";

describe("scenery tool", () => {
  it("previews and places the selected item with its direction", () => {
    const commands: SceneryCommand[] = [];
    const tool = createSceneryTool((command) => commands.push(command));
    tool.place("lamp", 1);
    tool.hover({ x: 3, y: 4, z: 8, quarter: 2 });
    expect(tool.state().preview).toEqual({ kind: "lamp", rotation: 1, x: 3, y: 4, z: 8, quarter: 2 });
    tool.select({ x: 3, y: 4, z: 8, quarter: 2 });
    expect(commands).toEqual([{ type: "scenery-place", kind: "lamp", x: 3, y: 4, rotation: 1, quarter: 2 }]);
    tool.cancel();
    expect(tool.state().preview).toBeNull();
  });

  it("removes the selected scenery and ignores empty tiles", () => {
    const commands: SceneryCommand[] = [];
    const tool = createSceneryTool((command) => commands.push(command));
    tool.remove();
    tool.select({ x: 1, y: 2, z: 8, quarter: 0 });
    expect(commands).toEqual([]);
    tool.select({ x: 1, y: 2, z: 8, quarter: 0 }, "1:2:8:0");
    expect(commands).toEqual([{ type: "scenery-remove", id: "1:2:8:0" }]);
  });

  it("orients directional artwork relative to the camera", () => {
    const sprites = scenerySprites([{ id: "1:2:8:0", kind: "lamp", x: 1, y: 2, z: 8, rotation: 3, quarter: 0 }], 1);
    expect(sprites[0]).toMatchObject({ sprite: "scenery/lamp", view: "rot180", height: 8, x: 1.25, y: 2.25 });
  });
});
