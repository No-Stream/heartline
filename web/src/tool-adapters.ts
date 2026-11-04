import type { InputAction } from "./input/actions";
import type { LandscapingSelection, LandscapingTile, createLandscapingTool } from "./landscaping-tool";
import type { PathSelection } from "./path-panel";
import type { PathTile, createPathTool } from "./path-tool";
import type { SceneryKind, SceneryTile, createSceneryTool } from "./scenery-tool";
import type { FacilityKind } from "./facilities";
import type { FacilityTile, createFacilityTool } from "./facility-tool";
import type { StationToolAction, createStationTool } from "./station-tool";
import type { DragPhase, InteractiveTool } from "./tool";

type PathActivation = { readonly mode: "build"; readonly selection: PathSelection } | { readonly mode: "remove" };
type SceneryActivation = { readonly mode: "place"; readonly kind: SceneryKind; readonly rotation: number } | { readonly mode: "remove" };
type FacilityActivation = { readonly mode: "place"; readonly kind: FacilityKind; readonly rotation: number } | { readonly mode: "remove" };
type SceneryPointer = { readonly tile: SceneryTile | null; readonly id: string | undefined };
type FacilityPointer = { readonly tile: FacilityTile | null; readonly id: number | undefined };
type StationPointer = Omit<StationToolAction, "type">;
const dragType = (phase: Exclude<DragPhase, "cancel">) => `drag-${phase}` as const;

export function pathToolAdapter(tool: ReturnType<typeof createPathTool>): InteractiveTool<PathActivation, PathTile | null, InputAction, ReturnType<typeof tool.state>> {
  return {
    state: tool.state,
    activate(selection) { if (selection.mode === "build") tool.build(selection.selection); else tool.remove(); },
    deactivate: tool.cancel,
    pointerMove: (tile) => tool.handle({ type: "hover", tile }),
    click: (tile) => tool.handle({ type: "select", tile }),
    drag: (phase, tile) => phase === "cancel" ? tool.handle({ type: "drag-cancel" }) : tool.handle({ type: dragType(phase), tile }),
    key: () => false,
  };
}

export function landscapingToolAdapter(tool: ReturnType<typeof createLandscapingTool>): InteractiveTool<LandscapingSelection, LandscapingTile | null, InputAction, ReturnType<typeof tool.state>> {
  return {
    state: tool.state, activate: tool.activate, deactivate: tool.cancel,
    pointerMove: (tile) => tool.handle({ type: "hover", tile }),
    click: (tile) => tool.handle({ type: "select", tile }),
    drag: (phase, tile) => phase === "cancel" ? tool.handle({ type: "drag-cancel" }) : tool.handle({ type: dragType(phase), tile }),
    key: () => false,
  };
}

export function sceneryToolAdapter(tool: ReturnType<typeof createSceneryTool>): InteractiveTool<SceneryActivation, SceneryPointer, InputAction, ReturnType<typeof tool.state>> {
  return {
    state: tool.state,
    activate(selection) { if (selection.mode === "place") tool.place(selection.kind, selection.rotation); else tool.remove(); },
    deactivate: tool.cancel,
    pointerMove(pointer) { if (tool.state().mode === "inactive") return false; tool.hover(pointer.tile); return true; },
    click(pointer) { if (tool.state().mode === "inactive") return false; tool.select(pointer.tile, pointer.id); return true; },
    drag: () => false, key: () => false,
  };
}

export function facilityToolAdapter(tool: ReturnType<typeof createFacilityTool>): InteractiveTool<FacilityActivation, FacilityPointer, InputAction, ReturnType<typeof tool.state>> {
  return {
    state: tool.state,
    activate(selection) { if (selection.mode === "place") tool.place(selection.kind, selection.rotation); else tool.remove(); },
    deactivate: tool.cancel,
    pointerMove: (pointer) => tool.handle({ type: "hover", tile: pointer.tile }),
    click: (pointer) => tool.handle({ type: "select", ...pointer }),
    drag: () => false, key: () => false,
  };
}

export function stationToolAdapter(tool: ReturnType<typeof createStationTool>): InteractiveTool<undefined, StationPointer, InputAction, ReturnType<typeof tool.state>> {
  return {
    state: tool.state, activate: tool.activate, deactivate: tool.cancel,
    pointerMove: (pointer) => tool.handle({ type: "hover", ...pointer }),
    click: (pointer) => tool.handle({ type: "select", ...pointer }),
    drag: () => false, key: () => false,
  };
}

export function coasterToolAdapter(handleKey: (key: InputAction) => boolean, handlePointer: (action: InputAction) => boolean = () => false): InteractiveTool<undefined, InputAction, InputAction, { readonly active: boolean }> {
  let active = false;
  return {
    state: () => ({ active }), activate: () => { active = true; }, deactivate: () => { active = false; },
    pointerMove: (action) => active && handlePointer(action), click: (action) => active && handlePointer(action), drag: (_phase, action) => active && handlePointer(action),
    key: (key) => active && handleKey(key),
  };
}
