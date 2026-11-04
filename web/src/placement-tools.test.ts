import { describe, expect, it } from "vitest";
import { createFacilityTool, type FacilityToolCommand } from "./facility-tool";
import { createStationTool, type StationToolCommand } from "./station-tool";

const tile = { x: 2, y: 3, height: 5.2, kind: "terrain" as const };

describe("placement tools", () => {
  it("previews, places and removes facilities, ignoring stale previews", () => {
    const commands: FacilityToolCommand[] = [];
    const tool = createFacilityTool((command) => commands.push(command));
    tool.place("food_stall", 2);
    tool.handle({ type: "hover", tile });
    const request = commands[0];
    expect(request).toEqual({ type: "facility-access-preview", requestId: 2, kind: "food_stall", rotation: 2, x: 2, y: 3, height: 5 });
    expect(tool.receivePreview(1, 0)).toBe(false);
    expect(tool.receivePreview(2, 1)).toBe(true);
    tool.handle({ type: "select", tile, id: undefined });
    expect(commands[1]).toEqual({ type: "facility-place", kind: "food_stall", rotation: 2, x: 2, y: 3, height: 5 });
    tool.placed();
    expect(tool.state().mode).toBe("inactive");
    tool.remove();
    tool.handle({ type: "select", tile: null, id: 4 });
    expect(commands[2]).toEqual({ type: "facility-remove", id: 4 });
  });

  it("locks the station position, chooses a direction and starts construction", () => {
    const commands: StationToolCommand[] = [];
    const tool = createStationTool((command) => commands.push(command));
    const project = (point: { x: number; y: number }) => point;
    tool.activate();
    tool.handle({ type: "select", position: { x: 2, y: 3 }, tile, preserve: false, project });
    expect(tool.state().station).toEqual({ x: 2, y: 3, z: 18, heading: 0, locked: true });
    tool.setHeight(14);
    tool.handle({ type: "select", position: { x: 2.5, y: 10 }, tile: null, preserve: false, project });
    expect(commands).toEqual([{ type: "construction-start", x: 2, y: 3, z: 20, heading: 1 }]);
    expect(tool.state().active).toBe(false);
  });
});
