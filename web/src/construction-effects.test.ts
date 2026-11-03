import { describe, expect, it } from "vitest";
import type { ConstructionState } from "./construction-protocol";
import { constructionEvent, createConstructionEffects } from "./construction-effects";

const rails = new Float32Array([0, 1, 1, 1, 0, 1, 0, 0, 0, 1, 2, 1, 1, 0, 1, 0, 0, 0, 1]);
const state = (update: Partial<ConstructionState>): ConstructionState => ({
  type: "construction-state", active: true, chain: false, pieces: [], selected: "Straight", track: [{ id: 0, label: "Station" }],
  canAppend: true, canClose: false, canTest: false, isOpen: false, coasterPrice: 6, coasterIncome: 0n, stationPositions: [], message: "",
  blockReport: { sections: [], trainCapacity: 0, capacitySearchComplete: null, theoreticalHourlyRiders: null, throughputStatus: "unavailable", placementStatus: "idle", placementSearchComplete: null, reasons: [], notes: [], suggestions: [] },
  trainCount: 1, placedRails: rails, railDistances: new Float32Array(), blockCandidates: new Float64Array(), placedTies: new Float32Array(), placedSupports: new Float32Array(),
  previewRails: rails, previewTies: new Float32Array(), previewSupports: new Float32Array(), ...update,
});
const style = { steel: "#dc5c3e", wood: "#d88440", train: "#9fbbb1", ink: "#3a2e58", lineWeight: 0.6 };
const viewport = { width: 800, height: 500 };
const camera = { centerX: 1, centerY: 1, zoom: 1, rotation: 0 as const };

describe("construction effects", () => {
  it("recognises a placed piece and a closed circuit, and ignores undo and refusals", () => {
    const one = state({});
    const two = state({ track: [...one.track, { id: 1, label: "Straight" }] });
    expect(constructionEvent(undefined, one)).toBeUndefined();
    expect(constructionEvent(one, two)).toBe("placed");
    expect(constructionEvent(two, one)).toBeUndefined();
    expect(constructionEvent(one, state({ message: "The train would collide with track." }))).toBeUndefined();
    expect(constructionEvent(state({ canClose: true }), state({ canClose: false, canTest: true, previewRails: new Float32Array() }))).toBe("closed");
  });

  it("draws a short-lived burst after a placement and confetti after closing", () => {
    const effects = createConstructionEffects(style);
    effects.observe(state({}), state({ track: [{ id: 0, label: "Station" }, { id: 1, label: "Straight" }] }), 1000);
    expect(effects.active(1100)).toBe(true);
    expect(effects.quads(1100, camera, viewport).some((quad) => quad.id.startsWith("effect-snap-"))).toBe(true);
    expect(effects.active(2000)).toBe(false);
    effects.observe(state({ canClose: true }), state({ canClose: false }), 3000);
    expect(effects.quads(3500, camera, viewport).filter((quad) => quad.id.startsWith("effect-confetti-"))).toHaveLength(90);
    expect(effects.active(6000)).toBe(false);
  });
});
