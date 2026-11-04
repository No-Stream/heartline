import { describe, expect, it } from "vitest";
import { createPathTool } from "./path-tool";
import { createSceneryTool } from "./scenery-tool";
import { pathToolAdapter, sceneryToolAdapter, coasterToolAdapter } from "./tool-adapters";
import type { InputAction } from "./input/actions";

it("routes pointer and drag events through the path tool", () => {
  const commands: unknown[] = [];
  const tool = pathToolAdapter(createPathTool((command) => commands.push(command)));
  tool.activate({ mode: "build", selection: { kind: "normal", style: 0 } });
  const tile = { x: 1, y: 2, z: 3, kind: "terrain" } as const;
  expect(tool.drag("start", tile)).toBe(true);
  tool.drag("end", tile);
  expect(commands.map((command) => (command as { type: string }).type)).toEqual(["path-preview", "path-build"]);
  tool.deactivate();
  expect(tool.click(tile)).toBe(false);
});

it("routes scenery selection through its existing tool", () => {
  const commands: unknown[] = [];
  const tool = sceneryToolAdapter(createSceneryTool((command) => commands.push(command)));
  tool.activate({ mode: "place", kind: "bench", rotation: 1 });
  const pick = { tile: { x: 1, y: 2, z: 3, quarter: 0 }, id: undefined };
  tool.pointerMove(pick);
  tool.click(pick);
  expect(commands).toEqual([{ type: "scenery-place", kind: "bench", rotation: 1, x: 1, y: 2, quarter: 0 }]);
});

it("forwards coaster keyboard actions only while active", () => {
  const actions: InputAction[] = [];
  const tool = coasterToolAdapter((action) => { actions.push(action); return true; }, (action) => { actions.push(action); return true; });
  const action = { type: "chain-toggle" } as const;
  expect(tool.key(action)).toBe(false);
  tool.activate(undefined);
  expect(tool.key(action)).toBe(true);
  const drag = { type: "block-marker-drag-start", position: { x: 1, y: 2 } } as const;
  expect(tool.drag("start", drag)).toBe(true);
  expect(actions).toEqual([action, drag]);
  tool.deactivate();
  expect(tool.key(action)).toBe(false);
});
