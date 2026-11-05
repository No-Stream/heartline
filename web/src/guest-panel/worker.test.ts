import { afterEach, expect, it, vi } from "vitest";
import { validateBlockAnalysisBudget } from "../game-test";
import type { ConstructionState } from "../construction-protocol";
import type { WorkerAction, WorkerMessage } from "../protocol";

const wasmMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  construct: vi.fn(),
  tick: vi.fn(),
  pieceShapes: vi.fn(() => "[]"),
  pathPreview: vi.fn(),
  pathBuild: vi.fn(),
  pathRemove: vi.fn(),
  constructionActive: vi.fn(() => false),
  setChain: vi.fn(),
  setLaunchIntensity: vi.fn(),
  constructionSelect: vi.fn(),
  constructionAppend: vi.fn(),
  constructionUndo: vi.fn(),
  constructionDelete: vi.fn(),
  constructionTrackLength: 0,
  finishAvailable: false,
  finishInvalidate: vi.fn(),
  finishSearchStart: vi.fn(),
  finishSearchAdvance: vi.fn(() => true),
  constructionTrainPoses: vi.fn(() => new Float32Array()),
  coasterTrainStates: vi.fn(() => new Float32Array()),
  constructionTrainStates: vi.fn(() => new Float32Array()),
  advanceBlockAnalysis: vi.fn((_budget: number) => false),
  advanceBlockPlacement: vi.fn((_budget: number) => {}),
  blockReport: vi.fn(() => '{"sections":[],"trainCapacity":0,"capacitySearchComplete":null,"theoreticalHourlyRiders":null,"throughputStatus":"unavailable","placementStatus":"idle","placementSearchComplete":null,"reasons":[],"notes":[],"suggestions":[]}'),
}));

it("rejects block analysis budgets outside the supported range", () => {
  expect(() => validateBlockAnalysisBudget(1)).not.toThrow();
  expect(() => validateBlockAnalysisBudget(1000)).not.toThrow();
  for (const budget of [0, 1001, 1.5, NaN]) {
    expect(() => validateBlockAnalysisBudget(budget)).toThrow(RangeError);
  }
});

vi.mock("../wasm/theme_park_sim_wasm.js", () => ({
  default: wasmMock.initialize,
  piece_shapes_json: wasmMock.pieceShapes,
  WasmSimulation: class {
    chain = false;
    carsPerTrain = 6;
    colors = JSON.stringify({ track: "coral", supports: "timber", trains: "sage" });
    constructor(seed: bigint, guestCount: number, park: string) { wasmMock.construct(seed, guestCount, park); }
    scenery_json() { return "[]"; }
    facilities_json() { return "[]"; }
    facility_revenue() { return 0n; }
    place_facility() { return true; }
    remove_facility() { return true; }
    facility_message() { return ""; }
    world_dimensions() { return new Uint16Array([3, 3]); }
    world_surfaces() { return new Int32Array(54); }
    world_paths() { return new Int32Array(); }
    world_path_metadata() { return new Int32Array(); }
    world_facility_openings() { return new Int32Array(); }
    world_carved() { return new Int32Array(); }
    path_preview(coordinates: Int32Array, kind: string, style: number) { return wasmMock.pathPreview(coordinates, kind, style); }
    path_build(coordinates: Int32Array, kind: string, style: number) { return wasmMock.pathBuild(coordinates, kind, style); }
    path_remove(x: number, y: number, z: number) { return wasmMock.pathRemove(x, y, z); }
    path_message() { return "Stale path command message"; }
    tick() { wasmMock.tick(); }
    tick_count() { return BigInt(wasmMock.tick.mock.calls.length); }
    guest_ids() { return new Uint32Array(); }
    guest_detail_json(id: number) { return JSON.stringify({ id: `guest-${id}`, name: `Guest ${id}` }); }
    snapshot() { return new Float32Array(); }
    guest_appearances() { return new Uint8Array(); }
    coaster_geometry() { return new Float32Array(); }
    coaster_rails() { return new Float32Array(); }
    coaster_ties() { return new Float32Array(); }
    coaster_supports() { return new Float32Array(); }
    coaster_colors() { return JSON.stringify({ track: "coral", supports: "timber", trains: "sage" }); }
    coaster_train_layout() { return JSON.stringify({ model: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 }); }
    coaster_train_poses() { return new Float32Array(); }
    coaster_train_states() { return wasmMock.coasterTrainStates(); }
    construction_riders() { return new Uint32Array(); }
    construction_message() { return ""; }
    construction_coaster_price() { return 0; }
    construction_coaster_income() { return 0; }
    construction_block_report_json() { return wasmMock.blockReport(); }
    construction_train_count() { return 1; }
    construction_min_cars_per_train() { return wasmMock.constructionActive() ? 3 : 0; }
    construction_max_cars_per_train() { return wasmMock.constructionActive() ? 6 : 0; }
    construction_set_cars_per_train(cars: number) {
      if (!wasmMock.constructionActive() || cars < 3 || cars > 6) return false;
      this.carsPerTrain = cars;
      return true;
    }
    construction_start() { wasmMock.constructionActive.mockReturnValue(true); wasmMock.constructionTrackLength = 1; }
    construction_select(piece: string) { wasmMock.constructionSelect(piece); }
    construction_append() { wasmMock.constructionAppend(); wasmMock.constructionTrackLength += 1; }
    construction_undo() { wasmMock.constructionUndo(); wasmMock.constructionTrackLength -= 1; }
    construction_delete(id: number) { wasmMock.constructionDelete(id); wasmMock.constructionTrackLength -= 1; }
    construction_advance_block_analysis(budget: number) { return wasmMock.advanceBlockAnalysis(budget); }
    construction_advance_block_placement_for_test(budget: number) { wasmMock.advanceBlockPlacement(budget); }
    construction_choices_with_preview(_entrySpeed: number | undefined, _status: string) { return "[]"; }
    construction_elements() { return "[]"; }
    construction_selected() { return ""; }
    construction_track() { return JSON.stringify(Array.from({ length: wasmMock.constructionTrackLength }, (_, id) => ({ id, label: "Straight", name: "Straight", group: "straight", slope: "level", kind: "track" }))); }
    construction_rails() { return new Float32Array(); }
    construction_ties() { return new Float32Array(); }
    construction_supports() { return new Float32Array(); }
    construction_active() { return wasmMock.constructionActive(); }
    construction_chain() { return this.chain; }
    construction_set_chain(enabled: boolean) { wasmMock.setChain(enabled); this.chain = enabled; }
    construction_set_launch_intensity(id: number, targetSpeed: number) { wasmMock.setLaunchIntensity(id, targetSpeed); return true; }
    construction_colors() { return wasmMock.constructionActive() ? this.colors : "null"; }
    construction_train_layout() { return wasmMock.constructionActive() ? JSON.stringify({ model: "velocity_viper", carsPerTrain: this.carsPerTrain, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: this.carsPerTrain * 3 }) : "null"; }
    construction_set_colors(track: string, supports: string, trains: string) { this.colors = JSON.stringify({ track, supports, trains }); }
    construction_train_poses() { return wasmMock.constructionTrainPoses(); }
    construction_train_states() { return wasmMock.constructionTrainStates(); }
    construction_placed_rails() { return new Float32Array(); }
    construction_rail_distances() { return new Float32Array(); }
    construction_block_candidates() { return new Float64Array(); }
    construction_placed_ties() { return new Float32Array(); }
    construction_placed_supports() { return new Float32Array(); }
    construction_can_close() { return false; }
    construction_test_report_json() { return "null"; }
    construction_force_preview_json() { return "null"; }
    construction_candidate_force_preview_json() { return "null"; }
    construction_speed_match_json() { return "null"; }
    construction_hill_options() { return "[]"; }
    construction_can_test() { return false; }
    construction_finish_preview_active() { return false; }
    construction_can_finish() { return wasmMock.finishAvailable; }
    construction_finish_reason() { return wasmMock.finishAvailable ? "" : "Checking for a connection to the station."; }
    construction_finish_search_start() { wasmMock.finishSearchStart(); wasmMock.finishAvailable = false; }
    construction_finish_search_advance() { return wasmMock.finishSearchAdvance(); }
    construction_finish_invalidate() { wasmMock.finishInvalidate(); wasmMock.finishAvailable = false; }
    construction_finish_preview() {}
    construction_finish_confirm() {}
    construction_finish_cancel() {}
    construction_is_open() { return false; }
    construction_station_positions() { return new Int32Array(); }
  },
}));

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.resetModules();
  wasmMock.tick.mockReset();
  wasmMock.initialize.mockReset();
  wasmMock.construct.mockReset();
  wasmMock.pathPreview.mockReset();
  wasmMock.pathBuild.mockReset();
  wasmMock.pathRemove.mockReset();
  wasmMock.constructionTrainPoses.mockReset().mockReturnValue(new Float32Array());
  wasmMock.coasterTrainStates.mockReset().mockReturnValue(new Float32Array());
  wasmMock.constructionTrainStates.mockReset().mockReturnValue(new Float32Array());
  wasmMock.advanceBlockAnalysis.mockReset().mockReturnValue(false);
  wasmMock.advanceBlockPlacement.mockReset();
  wasmMock.blockReport.mockReset().mockReturnValue('{"sections":[],"trainCapacity":0,"capacitySearchComplete":null,"theoreticalHourlyRiders":null,"throughputStatus":"unavailable","placementStatus":"idle","placementSearchComplete":null,"reasons":[],"notes":[],"suggestions":[]}');
  wasmMock.constructionActive.mockReset().mockReturnValue(false);
  wasmMock.setChain.mockReset();
  wasmMock.setLaunchIntensity.mockReset();
  wasmMock.constructionSelect.mockReset();
  wasmMock.constructionAppend.mockReset();
  wasmMock.constructionUndo.mockReset();
  wasmMock.constructionDelete.mockReset();
  wasmMock.constructionTrackLength = 0;
  wasmMock.finishAvailable = false;
  wasmMock.finishInvalidate.mockReset();
  wasmMock.finishSearchStart.mockReset();
  wasmMock.finishSearchAdvance.mockReset().mockReturnValue(true);
});

it("sends an incomplete station state before the complete state", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  postMessage.mockClear();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state").map(([message]) => message.complete)).toEqual([false, true]);
  const sent = postMessage.mock.calls.map(([message]) => message);
  expect(sent.findIndex((message) => message.type === "world")).toBeLessThan(sent.findIndex((message) => message.type === "construction-state" && message.complete));
});

it("sends block analysis progress without rebuilding construction geometry", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  wasmMock.blockReport.mockReturnValue('{"sections":[],"trainCapacity":2}');
  postMessage.mockClear();
  onMessage({ data: { type: "test-advance-block-analysis", budget: 1000 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.map(([message]) => message.type)).toEqual(["block-analysis-progress", "test-block-analysis-advanced"]);
  expect(postMessage.mock.calls[0][0]).toEqual({ type: "block-analysis-progress", blockReport: { sections: [], trainCapacity: 2 }, trainCount: 1, message: "" });
});

it("advances block analysis only when requested in test mode", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(1000);
  expect(wasmMock.advanceBlockAnalysis).not.toHaveBeenCalled();
  onMessage({ data: { type: "test-advance-block-analysis", budget: 42 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.advanceBlockAnalysis).toHaveBeenCalledExactlyOnceWith(42);
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "test-block-analysis-advanced" });
});

it("advances only placement on the dedicated frozen test command and records it", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "test-advance-block-placement", budget: 42 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.advanceBlockPlacement).toHaveBeenCalledExactlyOnceWith(42);
  expect(wasmMock.advanceBlockAnalysis).not.toHaveBeenCalled();
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "test-block-analysis-advanced" });
  onMessage({ data: { type: "export-session", requestId: 1 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.at(-1)?.[0].recording.commands).toEqual([{ tick: 0, command: { type: "test-advance-block-placement", budget: 42 } }]);
});

it("continues block analysis on a timer in live mode", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(70);
  expect(wasmMock.advanceBlockAnalysis).toHaveBeenCalledTimes(2);
  expect(wasmMock.advanceBlockAnalysis).toHaveBeenCalledWith(1000);
});

it("echoes the preview request id and reports invalid tiles", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.pathPreview.mockReturnValue(new Int32Array([0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 0, 0, 0, 0, 4, 8, 0]));
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "path-preview", requestId: 7, start: { x: 0, y: 0, z: 0 }, end: { x: 1, y: 0, z: 0 }, kind: "normal", style: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "path-preview", requestId: 7, valid: false, paths: expect.any(Int32Array), message: "Path route does not connect across the terrain" });
});

it("reports an out-of-world path preview without rendering the error record", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.pathPreview.mockReturnValue(Int32Array.of(-1, -1, 0, 0, 0, 0, 1, 0, 0));
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();

  onMessage({ data: { type: "path-preview", requestId: 8, start: { x: 0, y: 0, z: 0 }, end: { x: 3, y: 0, z: 0 }, kind: "normal", style: 0 } } as MessageEvent);
  await Promise.resolve();

  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "path-preview", requestId: 8, valid: false, paths: expect.any(Int32Array), message: "Path route is outside the park" });
});

it("echoes path command request ids so the controller can ignore stale results", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.pathBuild.mockReturnValue(true);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();

  onMessage({ data: { type: "path-build", requestId: 19, start: { x: 0, y: 0, z: 0 }, end: { x: 1, y: 0, z: 0 }, kind: "normal", style: 0 } } as MessageEvent);
  await Promise.resolve();

  expect(postMessage.mock.calls.at(-1)?.[0]).toMatchObject({
    type: "path-command-result",
    requestId: 19,
    command: "build",
    success: true,
  });
});

it("refreshes an open guest detail twice per second and stops after close", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", {
    postMessage,
    addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; },
  });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();

  onMessage({ data: { type: "guest-detail-request", guestId: "guest-10" } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({
    type: "guest-detail",
    guestId: "guest-10",
    detail: { id: "guest-10", name: "Guest 10" },
  });

  await vi.advanceTimersByTimeAsync(550);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "guest-detail")).toHaveLength(2);
  onMessage({ data: { type: "guest-detail-close" } } as MessageEvent);
  await Promise.resolve();
  const detailCountAfterClose = postMessage.mock.calls.filter(([message]) => message.type === "guest-detail").length;
  await vi.advanceTimersByTimeAsync(600);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "guest-detail")).toHaveLength(detailCountAfterClose);
});

it("refreshes the world after facilities are placed or removed", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  postMessage.mockClear();
  onMessage({ data: { type: "facility-place", kind: "carousel", x: 0, y: 0, rotation: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.some(([message]) => message.type === "world")).toBe(true);
  postMessage.mockClear();
  onMessage({ data: { type: "facility-remove", id: 1 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.some(([message]) => message.type === "world")).toBe(true);
});

it("does not request construction train poses or states before a coaster is under construction", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.constructionTrainPoses.mockImplementation(() => { throw new Error("Construction train does not exist"); });
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });

  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  await Promise.resolve();

  expect(wasmMock.constructionTrainPoses).not.toHaveBeenCalled();
  expect(wasmMock.constructionTrainStates).not.toHaveBeenCalled();
  expect(postMessage.mock.calls.some(([message]) => message.type === "snapshot" && message.constructionTrainPoses.length === 0)).toBe(true);
  expect(postMessage.mock.calls.some(([message]) => message.type === "snapshot" && message.constructionTrainStates.length === 0)).toBe(true);
});

it("includes per-train states in snapshots and transfers their buffers", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const trainStates = new Float32Array([0, 8, 1, 1.2, 0.4, 1]);
  wasmMock.coasterTrainStates.mockReturnValue(trainStates);
  const constructionTrainStates = new Float32Array([0, 3, 0, 1, 0, 1]);
  wasmMock.constructionTrainStates.mockReturnValue(constructionTrainStates);
  wasmMock.constructionActive.mockReturnValue(true);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });

  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  await Promise.resolve();

  const [snapshot, transfers] = postMessage.mock.calls.find(([message]) => message.type === "snapshot")!;
  expect(snapshot.trainStates).toBe(trainStates);
  expect(snapshot.constructionTrainStates).toBe(constructionTrainStates);
  expect(transfers).toContain(trainStates.buffer);
  expect(transfers).toContain(constructionTrainStates.buffer);
});

it("refreshes the world after coaster construction changes", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  postMessage.mockClear();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.some(([message]) => message.type === "world")).toBe(true);
});

it("sends chain changes to the simulation and publishes the resulting state", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0].chain).toBe(false);
  onMessage({ data: { type: "construction-set-chain", enabled: true } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0].chain).toBe(true);
  onMessage({ data: { type: "construction-set-chain", enabled: false } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0].chain).toBe(false);
  expect(wasmMock.setChain.mock.calls).toEqual([[true], [false]]);
});

it("records launch intensity commands with their target speed for session replay", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "new", guestCount: 1, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "construction-set-launch-intensity", id: 7, targetSpeed: 48.2 } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "export-session", requestId: 5 } } as MessageEvent);
  await Promise.resolve();

  expect(wasmMock.setLaunchIntensity.mock.calls).toEqual([[7, 48.2]]);
  const recording = postMessage.mock.calls.find(([message]) => message.type === "session-export")?.[0].recording;
  expect(recording.commands.at(-1)).toEqual({ tick: 0, command: { type: "construction-set-launch-intensity", id: 7, targetSpeed: 48.2 } });
});

it("publishes finish availability after track edits without invalidating it on selection", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.finishSearchAdvance.mockImplementation(() => {
    wasmMock.finishAvailable = true;
    return true;
  });
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 0, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();

  const dispatchAndFlush = async (action: WorkerAction): Promise<void> => {
    onMessage({ data: action } as MessageEvent);
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(0);
  };
  const latestConstructionState = (): ConstructionState | undefined => postMessage.mock.calls
    .map(([message]) => message as WorkerMessage)
    .filter((message): message is ConstructionState => message.type === "construction-state")
    .at(-1);

  await dispatchAndFlush({ type: "construction-start", x: 0, y: 0, z: 0, heading: 0 });
  expect(latestConstructionState()?.canFinish).toBe(true);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(1);

  await dispatchAndFlush({ type: "construction-select", piece: "Straight" });
  expect(latestConstructionState()?.canFinish).toBe(true);
  expect(wasmMock.finishInvalidate).toHaveBeenCalledTimes(1);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(1);

  const trackEdits: WorkerAction[] = [
    { type: "construction-append" },
    { type: "construction-undo" },
    { type: "construction-delete", id: 0 },
  ];
  for (const action of trackEdits) {
    await dispatchAndFlush(action);
    expect(latestConstructionState()?.canFinish).toBe(true);
  }
  expect(wasmMock.finishInvalidate).toHaveBeenCalledTimes(4);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(4);
});

it("sends coaster colours to the simulation and publishes them parsed, with the demo coaster's in the first world", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  const world = postMessage.mock.calls.find(([message]) => message.type === "world")?.[0];
  expect(world.coasterColors).toEqual({ track: "coral", supports: "timber", trains: "sage" });
  expect(world.coasterTrain).toEqual({ model: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  const construction = postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0];
  expect(construction.colors).toEqual({ track: "coral", supports: "timber", trains: "sage" });
  expect(construction.trainLayout).toEqual({ model: "velocity_viper", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });
  onMessage({ data: { type: "construction-set-colors", colors: { track: "teal", supports: "cream", trains: "plum" } } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0].colors).toEqual({ track: "teal", supports: "cream", trains: "plum" });
});


it("initializes the requested park and scales live ticks", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 99n, park: "new", guestCount: 25, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  await Promise.resolve();
  expect(wasmMock.construct).toHaveBeenCalledExactlyOnceWith(99n, 25, "new");
  onMessage({ data: { type: "set-speed", speed: 4 } } as MessageEvent);
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(100);
  expect(wasmMock.tick.mock.calls.length).toBeGreaterThanOrEqual(8);
});
