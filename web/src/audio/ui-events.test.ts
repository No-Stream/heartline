import { describe, expect, it } from "vitest";
import type { ConstructionState, ConstructionTrack } from "../construction-protocol";
import { soundFrame } from "./mapping";
import { constructionUiEvents, createUiEventQueue, pathUiEvent, placementUiEvent, terrainUiEvent } from "./ui-events";

const rails = new Float32Array([0, 1, 1, 1, 0, 1, 0, 0, 0, 1, 2, 1, 1, 0, 1, 0, 0, 0, 1]);
const piece = (id: number): ConstructionTrack => ({ id, label: "Straight", name: "Straight", group: "straight", slope: "Level", kind: "Track", launchTargetSpeed: null, launchMinimumSpeed: null, launchMaximumSpeed: null });
const state = (update: Partial<ConstructionState>): ConstructionState => ({
  type: "construction-state", complete: true, active: true, chain: false, colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "wooden_twister", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 }, pieces: [], elements: [], selected: "Straight", track: [piece(0)],
  canAppend: true, canClose: false, finishPreview: false, canFinish: false, finishReason: "", canTest: false, isOpen: false, coasterPrice: 6, coasterIncome: 0n, stationPositions: [], message: "",
  blockReport: { sections: [], trainCapacity: 0, capacitySearchComplete: null, theoreticalHourlyRiders: null, throughputStatus: "unavailable", placementStatus: "idle", placementSearchComplete: null, reasons: [], notes: [], suggestions: [] },
  trainCount: 1, placedRails: rails, railDistances: new Float32Array(), blockCandidates: new Float64Array(), placedTies: new Float32Array(), placedSupports: new Float32Array(),
  previewRails: rails, previewTies: new Float32Array(), previewSupports: new Float32Array(), constructionForcePreview: null, candidateForcePreview: null, speedMatch: null, hillOptions: [], ...update,
});

describe("builder sound triggers", () => {
  const one = state({});
  const two = state({ track: [piece(0), piece(1)] });

  it("plays a placement in the built coaster's material, an undo when track shrinks, and a flourish on closing", () => {
    expect(constructionUiEvents(one, two)).toEqual([{ sound: "place", material: "wood" }]);
    expect(constructionUiEvents(one, state({ track: two.track, trainLayout: { ...two.trainLayout!, model: "velocity_viper" } }))).toEqual([{ sound: "place", material: "steel" }]);
    expect(constructionUiEvents(two, one)).toEqual([{ sound: "undo" }]);
    expect(constructionUiEvents(state({ canClose: true }), state({ canClose: false, canTest: true }))).toEqual([{ sound: "finish" }]);
    expect(constructionUiEvents(state({ finishPreview: true }), state({ finishPreview: false, canTest: true, track: two.track }))).toEqual([{ sound: "finish" }]);
  });

  it("plays an error for a new refusal but not for progress messages, repeats or unrelated updates", () => {
    expect(constructionUiEvents(one, state({ message: "No chain lift on this piece" }))).toEqual([{ sound: "error" }]);
    expect(constructionUiEvents(state({ message: "No chain lift on this piece" }), state({ message: "No chain lift on this piece" }))).toEqual([]);
    expect(constructionUiEvents(one, state({ message: "Test train running." }))).toEqual([]);
    expect(constructionUiEvents(one, state({ message: "Review the connection, then confirm to finish the coaster." }))).toEqual([]);
    expect(constructionUiEvents(state({ message: "No chain lift on this piece" }), one)).toEqual([]);
    expect(constructionUiEvents(one, state({ chain: true }))).toEqual([]);
  });

  it("stays silent on the first state and while the builder is inactive", () => {
    expect(constructionUiEvents(undefined, two)).toEqual([]);
    expect(constructionUiEvents(state({ active: false }), two)).toEqual([]);
    expect(constructionUiEvents(two, state({ active: false, track: [] }))).toEqual([]);
  });
});

describe("tool sound triggers", () => {
  it("maps path, scenery, facility and terrain results to soft build, removal and error sounds", () => {
    expect(pathUiEvent({ command: "build", success: true })).toEqual({ sound: "build" });
    expect(pathUiEvent({ command: "remove", success: true })).toEqual({ sound: "undo" });
    expect(pathUiEvent({ command: "build", success: false })).toEqual({ sound: "error" });
    expect(placementUiEvent("placed")).toEqual({ sound: "build" });
    expect(placementUiEvent("removed")).toEqual({ sound: "undo" });
    expect(placementUiEvent("refused")).toEqual({ sound: "error" });
    expect(placementUiEvent(null)).toBeNull();
    expect(terrainUiEvent({ success: false })).toEqual({ sound: "error" });
    expect(terrainUiEvent({ success: true })).toBeNull();
  });
});

describe("interface sound queue", () => {
  it("drops a repeat of the same sound within 60 ms and passes queued events into the next sound frame", () => {
    const queue = createUiEventQueue();
    queue.push({ sound: "place", material: "steel" }, 1000);
    queue.push({ sound: "place", material: "steel" }, 1040);
    queue.push({ sound: "error" }, 1040);
    queue.push({ sound: "place", material: "steel" }, 1061);
    const events = queue.drain();
    expect(events).toEqual([{ sound: "place", material: "steel" }, { sound: "error" }, { sound: "place", material: "steel" }]);
    expect(queue.drain()).toEqual([]);
    const frame = soundFrame({ snapshot: undefined, interpolation: 1, camera: { centerX: 0, centerY: 0, zoom: 1, rotation: 0 }, viewport: { width: 800, height: 600 }, dayPhase: 0.5, gameSpeed: 16, demoTrain: null, constructionTrain: null, crestBursts: [], ui: events, paused: false, timeSeconds: 0 }).frame;
    expect(frame.ui).toEqual(events);
    expect(frame.eventsMuted).toBe(true);
  });
});
