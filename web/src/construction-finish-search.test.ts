import { describe, expect, it } from "vitest";
import type { WorkerAction } from "./protocol";
import { shouldInvalidateConstructionFinishSearch } from "./construction-finish-search";

describe("construction finish search triggers", () => {
  it("refreshes after track geometry, terrain, and facility obstacle edits", () => {
    const actions: WorkerAction[] = [
      { type: "construction-start", x: 1, y: 2, z: 3, heading: 0 },
      { type: "construction-set-chain", enabled: true },
      { type: "construction-set-cars-per-train", cars: 4 },
      { type: "construction-append" },
      { type: "construction-undo" },
      { type: "construction-delete", id: 0 },
      { type: "construction-close" },
      { type: "set-surface", x: 1, y: 2, corners: Int16Array.of(1, 1, 1, 1) },
      { type: "terrain-edit", command: "raise_tiles", x: 1, y: 2, size: 1, corner: 0, steps: 1, height: 1, bottom: 0, top: 0 },
      { type: "facility-place", kind: "food_stall", x: 1, y: 2, rotation: 0 },
      { type: "facility-remove", id: 1 },
    ];
    for (const action of actions) {
      expect(shouldInvalidateConstructionFinishSearch(action)).toBe(true);
    }
  });

  it("keeps the current result for selection and unrelated edits", () => {
    const actions: WorkerAction[] = [
      { type: "construction-select", piece: "Straight" },
      { type: "construction-set-colors", colors: { track: "coral", supports: "slate", trains: "sky" } },
      { type: "construction-set-coaster-price", price: 6 },
      { type: "construction-set-open", open: true },
      { type: "construction-set-train-count", count: 1 },
      { type: "construction-move-block", index: 0, boundaryMetres: 10 },
      { type: "scenery-place", kind: "tree_oval" as const, x: 1, y: 2, rotation: 0, quarter: 0 },
      { type: "path-build", start: { x: 1, y: 1, z: 0 }, end: { x: 2, y: 1, z: 0 }, kind: "normal" as const, style: 0, requestId: 1 },
      { type: "set-speed", speed: 2 },
    ];
    for (const action of actions) {
      expect(shouldInvalidateConstructionFinishSearch(action)).toBe(false);
    }
  });

  it("lets finish confirmation consume its preview before restarting availability", () => {
    expect(shouldInvalidateConstructionFinishSearch({ type: "construction-finish-confirm" })).toBe(false);
  });
});
