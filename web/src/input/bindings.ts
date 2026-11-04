import { pieceGroups } from "../construction-choices";
import type { InputAction, ScreenPoint } from "./actions";

interface Shortcut {
  /** `KeyboardEvent.key` values, lower-cased. The first one is shown in hints. */
  readonly keys: readonly string[];
  readonly repeat: boolean;
  readonly action: InputAction;
  readonly markerAction?: InputAction;
}

/** Game keys. The toolbar and the panels read their key hints from here so they cannot drift from the bindings. */
export const builderShortcuts = {
  turnLeft: { keys: ["a"], repeat: true, action: { type: "piece-step", attribute: "turn", step: -1 } },
  turnRight: { keys: ["d"], repeat: true, action: { type: "piece-step", attribute: "turn", step: 1 } },
  slopeDown: { keys: ["s"], repeat: true, action: { type: "piece-step", attribute: "slope", step: -1 } },
  slopeUp: { keys: ["w"], repeat: true, action: { type: "piece-step", attribute: "slope", step: 1 } },
  bankLeft: { keys: ["z"], repeat: true, action: { type: "piece-step", attribute: "bank", step: -1 } },
  bankRight: { keys: ["x"], repeat: true, action: { type: "piece-step", attribute: "bank", step: 1 } },
  previousPiece: { keys: ["["], repeat: true, action: { type: "piece-step", attribute: "piece", step: -1 } },
  nextPiece: { keys: ["]"], repeat: true, action: { type: "piece-step", attribute: "piece", step: 1 } },
  place: { keys: ["enter", "b"], repeat: false, action: { type: "construction-append" } },
  undo: { keys: ["backspace"], repeat: false, action: { type: "construction-undo" } },
  chain: { keys: ["h"], repeat: false, action: { type: "chain-toggle" } },
  deleteMode: { keys: ["delete"], repeat: false, action: { type: "delete-mode-toggle" }, markerAction: { type: "block-marker-remove" } },
  panLeft: { keys: ["arrowleft"], repeat: true, action: { type: "pan", dx: 32, dy: 0 }, markerAction: { type: "block-marker-nudge", step: -1 } },
  panRight: { keys: ["arrowright"], repeat: true, action: { type: "pan", dx: -32, dy: 0 }, markerAction: { type: "block-marker-nudge", step: 1 } },
  panUp: { keys: ["arrowup"], repeat: true, action: { type: "pan", dx: 0, dy: 32 } },
  panDown: { keys: ["arrowdown"], repeat: true, action: { type: "pan", dx: 0, dy: -32 } },
  close: { keys: ["c"], repeat: false, action: { type: "construction-close" } },
  test: { keys: ["t"], repeat: false, action: { type: "construction-test" } },
  cancel: { keys: ["escape"], repeat: false, action: { type: "builder-cancel" } },
  pathBuild: { keys: ["p"], repeat: false, action: { type: "path-build-mode" } },
  pathRemove: { keys: ["r"], repeat: false, action: { type: "path-remove-mode" } },
  sessionExport: { keys: ["j"], repeat: false, action: { type: "session-export" } },
  gameSpeed: { keys: ["v"], repeat: false, action: { type: "game-speed-cycle" } },
  soundMute: { keys: ["m"], repeat: false, action: { type: "sound-mute-toggle" } },
  undergroundView: { keys: ["u"], repeat: false, action: { type: "underground-view-toggle" } },
  coasterTool: { keys: ["o"], repeat: false, action: { type: "tool-select", tool: "coaster" } },
  landscapeTool: { keys: ["l"], repeat: false, action: { type: "tool-select", tool: "landscape" } },
  sceneryTool: { keys: ["g"], repeat: false, action: { type: "tool-select", tool: "scenery" } },
  facilitiesTool: { keys: ["f"], repeat: false, action: { type: "tool-select", tool: "facilities" } },
  ...Object.fromEntries(pieceGroups.map((group, index) => [`group-${group}`, { keys: [String(index + 1)], repeat: false, action: { type: "piece-group", group } }])),
} as const satisfies Record<string, Shortcut>;

const keyNames: Record<string, string> = { enter: "↵", backspace: "⌫", delete: "Del", escape: "Esc", arrowleft: "←", arrowright: "→", arrowup: "↑", arrowdown: "↓" };
export function keyHint(shortcut: Shortcut): string {
  const key = shortcut.keys[0];
  return keyNames[key] ?? key.toUpperCase();
}

export function groupKeyHint(group: string): string {
  const shortcut = (Object.values(builderShortcuts) as Shortcut[]).find(({ action }) => action.type === "piece-group" && action.group === group);
  if (!shortcut) throw new Error(`No shortcut for piece group ${group}`);
  return keyHint(shortcut);
}

const shortcutsByKey = new Map<string, Shortcut>(Object.values(builderShortcuts as Record<string, Shortcut>).flatMap((shortcut) => shortcut.keys.map((key) => [key, shortcut] as const)));

/** Keys typed into a text field, and Enter or Space on a button, belong to that control rather than to the game. */
function controlOwnsKey(target: EventTarget | null, key: string): boolean {
  const element = target as { tagName?: string; type?: string; getAttribute?(name: string): string | null } | null;
  const tag = element?.tagName;
  if (tag === "SELECT" || tag === "TEXTAREA" || (tag === "INPUT" && element?.type !== "checkbox")) return true;
  const activates = key === "enter" || key === " ";
  return activates && (tag === "BUTTON" || tag === "INPUT" || tag === "SUMMARY" || tag === "A" || element?.getAttribute?.("role") === "radio");
}

/** Bind keyboard shortcuts on an element and everything inside it, such as the world canvas or the builder panel. */
export function bindKeyboard(target: HTMLElement, dispatch: (action: InputAction) => void, center: () => ScreenPoint, markerSelected: () => boolean = () => false): () => void {
  const key = (event: KeyboardEvent): void => {
    const name = event.key.toLowerCase();
    if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey || controlOwnsKey(event.target, name)) return;
    let action: InputAction | undefined;
    const shortcut = shortcutsByKey.get(name);
    if (shortcut) {
      if (!event.repeat || shortcut.repeat) action = markerSelected() && shortcut.markerAction ? shortcut.markerAction : shortcut.action;
    } else {
      switch (name) {
        case "q": if (!event.repeat) action = { type: "rotate", turns: -1 }; break;
        case "e": if (!event.repeat) action = { type: "rotate", turns: 1 }; break;
        case "+": case "=": action = { type: "zoom", factor: 1.1, anchor: center() }; break;
        case "-": action = { type: "zoom", factor: 1 / 1.1, anchor: center() }; break;
      }
    }
    if (shortcut || action) event.preventDefault();
    if (action) dispatch(action);
  };
  target.addEventListener("keydown", key);
  return () => target.removeEventListener("keydown", key);
}

/** Bind a canvas to actions. Pointer coordinates are scaled to its backing-store pixels. */
export function bindInput(canvas: HTMLCanvasElement, dispatch: (action: InputAction) => void, pathDragging: () => boolean = () => false, markerAt: (position: ScreenPoint) => boolean = () => false, markerSelected: () => boolean = () => false): () => void {
  let pointer: { id: number; start: ScreenPoint; last: ScreenPoint; dragging: boolean; path: boolean; marker: boolean } | undefined;
  const point = (event: MouseEvent): ScreenPoint => {
    const bounds = canvas.getBoundingClientRect();
    return { x: (event.clientX - bounds.left) * canvas.width / bounds.width, y: (event.clientY - bounds.top) * canvas.height / bounds.height };
  };
  const down = (event: PointerEvent): void => {
    if (pointer || event.button !== 0) return;
    const position = point(event);
    pointer = { id: event.pointerId, start: position, last: position, dragging: false, path: pathDragging(), marker: markerAt(position) };
    canvas.setPointerCapture(event.pointerId);
  };
  const move = (event: PointerEvent): void => {
    if (!pointer) { dispatch({ type: "hover", position: point(event) }); return; }
    if (pointer.id !== event.pointerId) return;
    const position = point(event);
    if (!pointer.dragging && Math.hypot(position.x - pointer.start.x, position.y - pointer.start.y) < 4) return;
    if (!pointer.dragging && pointer.marker) dispatch({ type: "block-marker-drag-start", position: pointer.start });
    else if (!pointer.dragging && pointer.path) dispatch({ type: "drag-start", position: pointer.start });
    pointer.dragging = true;
    if (pointer.marker) dispatch({ type: "block-marker-drag-move", position });
    else if (pointer.path) dispatch({ type: "drag-move", position });
    else dispatch({ type: "pan", dx: position.x - pointer.last.x, dy: position.y - pointer.last.y });
    pointer.last = position;
  };
  const up = (event: PointerEvent): void => {
    if (!pointer || pointer.id !== event.pointerId) return;
    if (!pointer.dragging) dispatch({ type: "select", position: point(event) });
    else if (pointer.marker) dispatch({ type: "block-marker-drag-end", position: point(event) });
    else if (pointer.path) dispatch({ type: "drag-end", position: point(event) });
    canvas.releasePointerCapture(event.pointerId);
    pointer = undefined;
  };
  const cancel = (event: PointerEvent): void => {
    if (pointer?.id === event.pointerId) {
      if (pointer.dragging && pointer.marker) dispatch({ type: "block-marker-drag-cancel" });
      else if (pointer.dragging && pointer.path) dispatch({ type: "drag-cancel" });
      pointer = undefined;
    }
  };
  const wheel = (event: WheelEvent): void => {
    event.preventDefault();
    const pixels = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? canvas.height : 1);
    dispatch({ type: "zoom", factor: Math.exp(-pixels * 0.001), anchor: point(event) });
  };
  const leave = (): void => { if (!pointer) dispatch({ type: "hover", position: null }); };
  canvas.addEventListener("pointerleave", leave);
  canvas.addEventListener("pointerdown", down);
  canvas.addEventListener("pointermove", move);
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", cancel);
  canvas.addEventListener("wheel", wheel, { passive: false });
  const unbindKeyboard = bindKeyboard(canvas, dispatch, () => ({ x: canvas.width / 2, y: canvas.height / 2 }), markerSelected);
  return () => {
    canvas.removeEventListener("pointerleave", leave);
    canvas.removeEventListener("pointerdown", down);
    canvas.removeEventListener("pointermove", move);
    canvas.removeEventListener("pointerup", up);
    canvas.removeEventListener("pointercancel", cancel);
    canvas.removeEventListener("wheel", wheel);
    unbindKeyboard();
  };
}
