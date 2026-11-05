import type { WorkerAction } from "./protocol";

/** Actions to invalidate before they can change completion geometry or its inputs. */
export function shouldInvalidateConstructionFinishSearch(action: WorkerAction): boolean {
  switch (action.type) {
    case "construction-start":
    case "construction-set-chain":
    case "construction-set-cars-per-train":
    case "construction-append":
    case "construction-undo":
    case "construction-delete":
    case "construction-close":
    case "set-surface":
    case "terrain-edit":
    case "facility-place":
    case "facility-remove":
      return true;
    default:
      return false;
  }
}
