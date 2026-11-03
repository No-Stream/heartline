import type { PathSelection } from "./path-panel";

export interface PathPosition { readonly x: number; readonly y: number; readonly z: number }
export interface PathTile extends PathPosition { readonly kind: "terrain" | "path" }

export type PathToolAction =
  | { readonly type: "hover" | "select" | "drag-start" | "drag-move" | "drag-end"; readonly tile: PathTile | null }
  | { readonly type: "drag-cancel" };

export type PathToolCommand =
  | { readonly type: "path-preview"; readonly start: PathPosition; readonly end: PathPosition; readonly kind: PathSelection["kind"]; readonly style: number; readonly requestId: number }
  | { readonly type: "path-build"; readonly start: PathPosition; readonly end: PathPosition; readonly kind: PathSelection["kind"]; readonly style: number; readonly requestId: number }
  | ({ readonly type: "path-remove"; readonly requestId: number } & PathPosition);

export interface PathToolPreview {
  readonly valid: boolean;
  /** Nine integers per tile: x, y, height, slope, kind, style, refusal code, path edge mask, facility opening mask. */
  readonly paths: Int32Array;
  readonly message: string;
}
export type PathToolMessage =
  | ({ readonly type: "path-preview"; readonly requestId: number } & PathToolPreview)
  | { readonly type: "path-command-result"; readonly requestId: number; readonly command: "build" | "remove"; readonly success: boolean; readonly message: string };
export interface PathToolState {
  readonly mode: "inactive" | "build" | "remove";
  readonly preview: PathToolPreview | null;
  readonly message: string;
}

function position(tile: PathTile): PathPosition {
  return { x: tile.x, y: tile.y, z: tile.z };
}

/** Coordinates are picked by the caller; pointer and keyboard bindings can share these actions. */
export function createPathTool(send: (command: PathToolCommand) => void, changed: () => void = () => {}) {
  let mode: PathToolState["mode"] = "inactive";
  let selection: PathSelection = { kind: "normal", style: 0 };
  let dragStart: PathPosition | null = null;
  let preview: PathToolPreview | null = null;
  let message = "";
  let requestId = 0;
  let pendingPreview: number | null = null;
  let commandRequestId = 0;
  let pendingCommand: number | null = null;

  function clearPreview(): void {
    pendingPreview = null;
    preview = null;
  }

  function requestPreview(start: PathPosition, end: PathPosition): void {
    requestId += 1;
    pendingPreview = requestId;
    preview = null;
    send({ type: "path-preview", start, end, ...selection, requestId });
    changed();
  }

  function buildRoute(start: PathPosition, end: PathPosition): void {
    clearPreview();
    commandRequestId += 1;
    pendingCommand = commandRequestId;
    send({ type: "path-build", start, end, ...selection, requestId: commandRequestId });
    changed();
  }

  function removePath(tile: PathTile): void {
    commandRequestId += 1;
    pendingCommand = commandRequestId;
    send({ type: "path-remove", ...position(tile), requestId: commandRequestId });
    changed();
  }

  return {
    state(): PathToolState { return { mode, preview, message }; },
    build(next: PathSelection): void {
      selection = { ...next };
      mode = "build";
      dragStart = null;
      clearPreview();
      pendingCommand = null;
      message = "Drag between tiles to build a path.";
      changed();
    },
    remove(): void {
      mode = "remove";
      dragStart = null;
      clearPreview();
      pendingCommand = null;
      message = "Choose a path to remove.";
      changed();
    },
    cancel(): void {
      mode = "inactive";
      dragStart = null;
      clearPreview();
      pendingCommand = null;
      message = "Path selection cancelled.";
      changed();
    },
    handle(action: PathToolAction): boolean {
      if (mode === "inactive") return false;
      if (action.type === "drag-cancel") {
        dragStart = null;
        clearPreview();
        changed();
        return true;
      }
      if (mode === "remove") {
        if (action.type === "select") {
          if (action.tile?.kind === "path") removePath(action.tile);
          else message = "Choose a path to remove.";
          changed();
        }
        return true;
      }
      const tile = action.tile ? position(action.tile) : null;
      if (action.type === "select") {
        if (tile) buildRoute(tile, tile);
      } else if (action.type === "drag-start") {
        dragStart = tile;
        if (tile) requestPreview(tile, tile);
      } else if (action.type === "drag-end") {
        if (dragStart && tile) buildRoute(dragStart, tile);
        else { clearPreview(); changed(); }
        dragStart = null;
      } else if (tile) {
        requestPreview(dragStart ?? tile, tile);
      } else {
        clearPreview();
        changed();
      }
      return true;
    },
    receive(result: PathToolMessage): void {
      if (result.type === "path-preview") {
        if (result.requestId !== pendingPreview || mode !== "build") return;
        pendingPreview = null;
        preview = { valid: result.valid, paths: result.paths, message: result.message };
        message = result.message;
      } else {
        if (result.requestId !== pendingCommand) return;
        pendingCommand = null;
        message = result.message;
      }
      changed();
    },
  };
}
