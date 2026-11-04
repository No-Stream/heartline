import type { FacilityKind } from "./facilities";
import type { Tool } from "./tool";

export interface FacilityTile { readonly x: number; readonly y: number; readonly height: number }
export type FacilityToolAction =
  | { readonly type: "hover"; readonly tile: FacilityTile | null }
  | { readonly type: "select"; readonly tile: FacilityTile | null; readonly id: number | undefined };
export type FacilityToolCommand =
  | ({ readonly type: "facility-access-preview"; readonly requestId: number; readonly kind: FacilityKind; readonly rotation: number } & FacilityTile)
  | ({ readonly type: "facility-place"; readonly kind: FacilityKind; readonly rotation: number } & FacilityTile)
  | { readonly type: "facility-remove"; readonly id: number };
export interface FacilityToolState {
  readonly mode: "inactive" | "place" | "remove";
  readonly message: string;
}

export function createFacilityTool(send: (command: FacilityToolCommand) => void, changed: () => void = () => {}): Tool<FacilityToolAction, FacilityToolState> & {
  place(kind: FacilityKind, rotation: number): void;
  remove(): void;
  placed(): void;
  receivePreview(requestId: number, rotation: number | undefined): boolean;
} {
  let mode: FacilityToolState["mode"] = "inactive";
  let selection: { kind: FacilityKind; rotation: number } | undefined;
  let requestId = 0;
  let message = "";
  return {
    state: () => ({ mode, message }),
    place(kind, rotation) { requestId++; selection = { kind, rotation }; mode = "place"; message = ""; changed(); },
    remove() { selection = undefined; mode = "remove"; message = ""; changed(); },
    cancel() { selection = undefined; mode = "inactive"; changed(); },
    placed() { selection = undefined; mode = "inactive"; changed(); },
    receivePreview(id) { return mode === "place" && id === requestId; },
    handle(action) {
      if (mode === "inactive") return false;
      if (action.type === "hover") {
        if (selection) {
          requestId++;
          if (action.tile) send({ type: "facility-access-preview", requestId, ...selection, x: action.tile.x, y: action.tile.y, height: Math.round(action.tile.height) });
          else { message = ""; changed(); }
        }
      } else if (mode === "remove") {
        if (action.id !== undefined) send({ type: "facility-remove", id: action.id });
        else { message = "Choose a facility to remove."; changed(); }
      } else if (selection && action.tile) {
        send({ type: "facility-place", ...selection, x: action.tile.x, y: action.tile.y, height: Math.round(action.tile.height) });
      } else { message = "Choose a flat terrain tile."; changed(); }
      return true;
    },
  };
}
