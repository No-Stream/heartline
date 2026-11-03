import { describe, expect, it } from "vitest";
import { createPathTool, type PathToolCommand } from "./path-tool";

const first = { x: 1, y: 2, z: 8, kind: "terrain" } as const;
const last = { x: 4, y: 3, z: 9, kind: "terrain" } as const;
const existing = { x: 4, y: 3, z: 9, kind: "path" } as const;

function setup() {
  const commands: PathToolCommand[] = [];
  const tool = createPathTool((command) => commands.push(command));
  return { tool, commands };
}

describe("path tool", () => {
  it("previews a drag and builds its route with the chosen kind and style", () => {
    const { tool, commands } = setup();
    tool.build({ kind: "queue", style: 2 });
    expect(tool.handle({ type: "drag-start", tile: first })).toBe(true);
    tool.handle({ type: "drag-move", tile: last });
    expect(commands).toEqual([
      { type: "path-preview", start: firstPosition(), end: firstPosition(), kind: "queue", style: 2, requestId: 1 },
      { type: "path-preview", start: firstPosition(), end: lastPosition(), kind: "queue", style: 2, requestId: 2 },
    ]);
    tool.handle({ type: "drag-end", tile: last });
    expect(commands.at(-1)).toEqual({ type: "path-build", start: firstPosition(), end: lastPosition(), kind: "queue", style: 2, requestId: 1 });
  });

  it("places a single tile and removes only a selected path", () => {
    const { tool, commands } = setup();
    tool.build({ kind: "normal", style: 0 });
    tool.handle({ type: "select", tile: first });
    expect(commands).toEqual([{ type: "path-build", start: firstPosition(), end: firstPosition(), kind: "normal", style: 0, requestId: 1 }]);
    tool.remove();
    tool.handle({ type: "select", tile: first });
    expect(tool.state().message).toBe("Choose a path to remove.");
    tool.handle({ type: "select", tile: existing });
    expect(commands.at(-1)).toEqual({ type: "path-remove", ...lastPosition(), requestId: 2 });
  });

  it("ignores stale previews after a newer hover, a build, or cancellation", () => {
    const { tool } = setup();
    tool.build({ kind: "normal", style: 0 });
    tool.handle({ type: "hover", tile: first });
    tool.handle({ type: "hover", tile: last });
    const old = Int32Array.of(1, 2, 8, 0, 0, 0, 0);
    const current = Int32Array.of(4, 3, 9, 0, 0, 0, 1);
    tool.receive({ type: "path-preview", requestId: 1, valid: true, paths: old, message: "" });
    expect(tool.state().preview).toBeNull();
    tool.receive({ type: "path-preview", requestId: 2, valid: false, paths: current, message: "Too steep" });
    expect(tool.state().preview).toEqual({ valid: false, paths: current, message: "Too steep" });
    tool.handle({ type: "select", tile: last });
    expect(tool.state().preview).toBeNull();
    tool.receive({ type: "path-preview", requestId: 2, valid: false, paths: current, message: "Too steep" });
    expect(tool.state().preview).toBeNull();
    tool.handle({ type: "hover", tile: first });
    tool.cancel();
    tool.receive({ type: "path-preview", requestId: 3, valid: true, paths: old, message: "" });
    expect(tool.state()).toMatchObject({ mode: "inactive", preview: null });
  });

  it("ignores a late command result after cancellation or a mode change", () => {
    const { tool, commands } = setup();
    tool.build({ kind: "normal", style: 0 });
    tool.handle({ type: "select", tile: first });
    const buildCommand = commands.at(-1);
    expect(buildCommand?.type).toBe("path-build");
    expect(buildCommand?.requestId).toBe(1);
    tool.cancel();
    const stateAfterCancel = tool.state();
    tool.receive({
      type: "path-command-result",
      requestId: 1,
      command: "build",
      success: true,
      message: "Path built.",
    });
    expect(tool.state()).toEqual(stateAfterCancel);

    tool.remove();
    tool.handle({ type: "select", tile: existing });
    const removeCommand = commands.at(-1);
    expect(removeCommand?.type).toBe("path-remove");
    expect(removeCommand?.requestId).toBe(2);
    tool.build({ kind: "queue", style: 1 });
    const stateAfterModeChange = tool.state();
    tool.receive({
      type: "path-command-result",
      requestId: 2,
      command: "remove",
      success: true,
      message: "Path removed.",
    });
    expect(tool.state()).toEqual(stateAfterModeChange);
  });

  it("does not build a cancelled drag or a drag ending outside the world", () => {
    const { tool, commands } = setup();
    expect(tool.handle({ type: "select", tile: first })).toBe(false);
    tool.build({ kind: "normal", style: 0 });
    tool.handle({ type: "drag-start", tile: first });
    tool.handle({ type: "drag-cancel" });
    tool.handle({ type: "drag-end", tile: last });
    tool.handle({ type: "drag-start", tile: first });
    tool.handle({ type: "drag-end", tile: null });
    expect(commands.every((command) => command.type !== "path-build")).toBe(true);
  });
});

function firstPosition() { return { x: 1, y: 2, z: 8 }; }
function lastPosition() { return { x: 4, y: 3, z: 9 }; }
