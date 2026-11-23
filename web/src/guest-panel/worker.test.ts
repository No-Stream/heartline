import { afterEach, expect, it, vi } from "vitest";
import { validateBlockAnalysisBudget } from "../game-test";
import type { ConstructionState } from "../construction-protocol";
import { type CoasterWorldMessage, type WorkerAction, type WorkerMessage } from "../protocol";
import { replaySessionIntoWorker } from "../session-replay-worker";
import type { ReplaySession } from "../session-replay";

const wasmMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  construct: vi.fn(),
  fromParkDescription: vi.fn(),
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
  constructionAppendResolved: vi.fn(),
  constructionUndo: vi.fn(),
  constructionDelete: vi.fn(),
  constructionStart: vi.fn(),
  constructionCanAppend: vi.fn(() => false),
  constructionPreviewReason: vi.fn(() => ""),
  builderInvalidated: vi.fn(),
  builderSelected: vi.fn(),
  builderEstimate: vi.fn(),
  constructionTrackLength: 0,
  finishConfirmCloses: false,
  finishAvailable: false,
  finishInvalidate: vi.fn(),
  finishSearchStart: vi.fn(),
  finishSearchAdvance: vi.fn(() => true),
  constructionTrainPoses: vi.fn(() => new Float32Array()),
  coasterRenderList: vi.fn(() => JSON.stringify([{ id: 0, trackRevision: "starter-1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } }])),
  coasterIds: vi.fn(() => Uint32Array.of(0)),
  editingCoasterId: vi.fn(() => null as number | null),
  coasterGeometry: vi.fn((id: number) => Float32Array.of(id)),
  coasterRails: vi.fn((id: number) => Float32Array.of(id, 1)),
  coasterTies: vi.fn((id: number) => Float32Array.of(id, 2)),
  coasterSupports: vi.fn((id: number) => Float32Array.of(id, 3)),
  coasterTrainPoses: vi.fn((_id: number) => new Float32Array()),
  coasterTrainStates: vi.fn((_id: number) => new Float32Array()),
  coasterRiders: vi.fn((_id: number) => new Uint32Array()),
  rideInfo: vi.fn((id: number) => JSON.stringify({ ride: id, modelName: "Timber Tumbler", manufacturer: "Vekoma", limits: {}, report: null })),
  rideTest: vi.fn((_id: number) => ""),
  testBeginQueueEntryCapture: vi.fn(),
  testTakeQueueEntryPlacesJson: vi.fn(() => "[]"),
  testGuestQueueDiagnostics: vi.fn(() => JSON.stringify({ guests: [], rides: [], facilityAccess: [], coasters: [] })),
  coastersJson: vi.fn(() => "[]"),
  coasterEdit: vi.fn((_id: number) => false),
  coasterStopEditing: vi.fn(),
  coasterDemolish: vi.fn((_id: number) => false),
  coasterRename: vi.fn((_id: number, _name: string) => false),
  coasterSetOpen: vi.fn((_id: number, _open: boolean) => false),
  coasterSetPrice: vi.fn((_id: number, _price: number) => false),
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
  BuilderSessionBridge: class {
    invalidate() {
      wasmMock.builderInvalidated();
    }
    start(simulation: { construction_start(x: number, y: number, z: number, heading: number): void }, x: number, y: number, z: number, heading: number) {
      this.invalidate();
      wasmMock.constructionStart(x, y, z, heading);
      simulation.construction_start(x, y, z, heading);
    }
    select(simulation: { construction_select(piece: string): void }, piece: string, exact: boolean) {
      wasmMock.builderSelected(piece, exact);
      simulation.construction_select(piece);
      return "";
    }
    estimates(_simulation: object, complete: boolean) {
      wasmMock.builderEstimate(complete);
      return JSON.stringify({ constructionForcePreview: null, pieces: [], elements: [], hillOptions: [], speedTurns: [], candidateForcePreview: null, selectedShape: null, speedMatch: null });
    }
    free() {}
  },
  WasmSimulation: class {
    chain = false;
    carsPerTrain = 6;
    colors = JSON.stringify({ track: "coral", supports: "timber", trains: "sage" });
    constructor(seed: bigint, guestCount: number, park: string) { wasmMock.construct(seed, guestCount, park); }
    static from_park_description(seed: bigint, guestCount: number, json: string) {
      wasmMock.fromParkDescription(seed, guestCount, json);
      return new this(seed, guestCount, "new");
    }
    static load_state(_bytes: Uint8Array) { return new this(0n, 0, "demo"); }
    free() {}
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
    coaster_render_list_json() { return wasmMock.coasterRenderList(); }
    coaster_ids() { return wasmMock.coasterIds(); }
    editing_coaster_id_value() { return wasmMock.editingCoasterId(); }
    coaster_geometry(id: number) { return wasmMock.coasterGeometry(id); }
    coaster_rails(id: number) { return wasmMock.coasterRails(id); }
    coaster_ties(id: number) { return wasmMock.coasterTies(id); }
    coaster_supports(id: number) { return wasmMock.coasterSupports(id); }
    coaster_train_poses(id: number) { return wasmMock.coasterTrainPoses(id); }
    coaster_train_states(id: number) { return wasmMock.coasterTrainStates(id); }
    coaster_riders(id: number) { return wasmMock.coasterRiders(id); }
    coaster_ride_info_json(id: number) { return wasmMock.rideInfo(id); }
    coaster_test(id: number) { return wasmMock.rideTest(id); }
    test_begin_queue_entry_capture() { wasmMock.testBeginQueueEntryCapture(); }
    test_take_queue_entry_places_json() { return wasmMock.testTakeQueueEntryPlacesJson(); }
    test_guest_queue_diagnostics_json() { return wasmMock.testGuestQueueDiagnostics(); }
    coasters_json() { return wasmMock.coastersJson(); }
    coaster_edit(id: number) { return wasmMock.coasterEdit(id); }
    coaster_stop_editing() { wasmMock.coasterStopEditing(); }
    coaster_demolish(id: number) { return wasmMock.coasterDemolish(id); }
    coaster_rename(id: number, name: string) { return wasmMock.coasterRename(id, name); }
    coaster_set_open(id: number, open: boolean) { return wasmMock.coasterSetOpen(id, open); }
    coaster_set_price(id: number, price: number) { return wasmMock.coasterSetPrice(id, price); }
    coaster_colors() { return JSON.stringify({ track: "coral", supports: "timber", trains: "sage" }); }
    coaster_train_layout() { return JSON.stringify({ model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 }); }
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
    construction_start(_x: number, _y: number, _z: number, _heading: number) {
      wasmMock.constructionActive.mockReturnValue(true);
      wasmMock.constructionTrackLength = 1;
    }
    construction_select(piece: string) { wasmMock.constructionSelect(piece); }
    construction_append() { wasmMock.constructionAppend(); wasmMock.constructionTrackLength += 1; }
    construction_append_resolved(intentJson: string, specJson: string) { wasmMock.constructionAppendResolved(intentJson, specJson); wasmMock.constructionTrackLength += 1; return ""; }
    construction_undo() { wasmMock.constructionUndo(); wasmMock.constructionTrackLength -= 1; }
    construction_delete(id: number) { wasmMock.constructionDelete(id); wasmMock.constructionTrackLength -= 1; }
    construction_advance_block_analysis(budget: number) { return wasmMock.advanceBlockAnalysis(budget); }
    construction_advance_block_placement_for_test(budget: number) { wasmMock.advanceBlockPlacement(budget); }
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
    construction_train_layout() { return wasmMock.constructionActive() ? JSON.stringify({ model: "velocity_viper", trackMaterial: "steel", carsPerTrain: this.carsPerTrain, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: this.carsPerTrain * 3 }) : "null"; }
    construction_set_colors(track: string, supports: string, trains: string) { this.colors = JSON.stringify({ track, supports, trains }); }
    construction_train_poses() { return wasmMock.constructionTrainPoses(); }
    construction_train_states() { return wasmMock.constructionTrainStates(); }
    construction_placed_rails() { return new Float32Array(); }
    construction_rail_distances() { return new Float32Array(); }
    construction_block_candidates() { return new Float64Array(); }
    construction_placed_ties() { return new Float32Array(); }
    construction_placed_supports() { return new Float32Array(); }
    construction_can_append() { return wasmMock.constructionCanAppend(); }
    construction_preview_reason() { return wasmMock.constructionPreviewReason(); }
    construction_can_close() { return false; }
    construction_test_report_json() { return "null"; }
    construction_can_test() { return false; }
    construction_finish_preview_active() { return false; }
    construction_can_finish() { return wasmMock.finishAvailable; }
    construction_finish_reason() { return wasmMock.finishAvailable ? "" : "Checking for a connection to the station."; }
    construction_finish_search_start() { wasmMock.finishSearchStart(); wasmMock.finishAvailable = false; }
    construction_finish_search_advance() { return wasmMock.finishSearchAdvance(); }
    construction_finish_options_json() { return JSON.stringify({ previewedIndex: null, options: [] }); }
    construction_finish_cycle() {}
    construction_finish_invalidate() { wasmMock.finishInvalidate(); wasmMock.finishAvailable = false; }
    construction_finish_preview() {}
    construction_finish_confirm() { if (wasmMock.finishConfirmCloses) wasmMock.constructionActive.mockReturnValue(false); }
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
  wasmMock.fromParkDescription.mockReset();
  wasmMock.pathPreview.mockReset();
  wasmMock.pathBuild.mockReset();
  wasmMock.pathRemove.mockReset();
  wasmMock.constructionTrainPoses.mockReset().mockReturnValue(new Float32Array());
  wasmMock.coasterRenderList.mockReset().mockReturnValue(JSON.stringify([{ id: 0, trackRevision: "starter-1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } }]));
  wasmMock.coasterIds.mockReset().mockReturnValue(Uint32Array.of(0));
  wasmMock.editingCoasterId.mockReset().mockReturnValue(null);
  wasmMock.coasterGeometry.mockReset().mockImplementation((id: number) => Float32Array.of(id));
  wasmMock.coasterRails.mockReset().mockImplementation((id: number) => Float32Array.of(id, 1));
  wasmMock.coasterTies.mockReset().mockImplementation((id: number) => Float32Array.of(id, 2));
  wasmMock.coasterSupports.mockReset().mockImplementation((id: number) => Float32Array.of(id, 3));
  wasmMock.coasterTrainPoses.mockReset().mockReturnValue(new Float32Array());
  wasmMock.coasterTrainStates.mockReset().mockReturnValue(new Float32Array());
  wasmMock.coasterRiders.mockReset().mockReturnValue(new Uint32Array());
  wasmMock.rideInfo.mockReset().mockImplementation((id: number) => JSON.stringify({ ride: id, modelName: "Timber Tumbler", manufacturer: "Vekoma", limits: {}, report: null }));
  wasmMock.rideTest.mockReset().mockReturnValue("");
  wasmMock.testBeginQueueEntryCapture.mockReset();
  wasmMock.testTakeQueueEntryPlacesJson.mockReset().mockReturnValue("[]");
  wasmMock.testGuestQueueDiagnostics.mockReset().mockReturnValue(JSON.stringify({ guests: [], rides: [], facilityAccess: [], coasters: [] }));
  wasmMock.coastersJson.mockReset().mockReturnValue("[]");
  wasmMock.coasterEdit.mockReset().mockReturnValue(false);
  wasmMock.coasterStopEditing.mockReset();
  wasmMock.coasterDemolish.mockReset().mockReturnValue(false);
  wasmMock.coasterRename.mockReset().mockReturnValue(false);
  wasmMock.coasterSetOpen.mockReset().mockReturnValue(false);
  wasmMock.coasterSetPrice.mockReset().mockReturnValue(false);
  wasmMock.constructionTrainStates.mockReset().mockReturnValue(new Float32Array());
  wasmMock.advanceBlockAnalysis.mockReset().mockReturnValue(false);
  wasmMock.advanceBlockPlacement.mockReset();
  wasmMock.blockReport.mockReset().mockReturnValue('{"sections":[],"trainCapacity":0,"capacitySearchComplete":null,"theoreticalHourlyRiders":null,"throughputStatus":"unavailable","placementStatus":"idle","placementSearchComplete":null,"reasons":[],"notes":[],"suggestions":[]}');
  wasmMock.constructionActive.mockReset().mockReturnValue(false);
  wasmMock.setChain.mockReset();
  wasmMock.setLaunchIntensity.mockReset();
  wasmMock.constructionSelect.mockReset();
  wasmMock.constructionAppend.mockReset();
  wasmMock.constructionAppendResolved.mockReset();
  wasmMock.constructionUndo.mockReset();
  wasmMock.constructionDelete.mockReset();
  wasmMock.constructionStart.mockReset();
  wasmMock.constructionCanAppend.mockReset().mockReturnValue(false);
  wasmMock.constructionPreviewReason.mockReset().mockReturnValue("");
  wasmMock.builderInvalidated.mockReset();
  wasmMock.builderSelected.mockReset();
  wasmMock.builderEstimate.mockReset();
  wasmMock.constructionTrackLength = 0;
  wasmMock.finishConfirmCloses = false;
  wasmMock.finishAvailable = false;
  wasmMock.finishInvalidate.mockReset();
  wasmMock.finishSearchStart.mockReset();
  wasmMock.finishSearchAdvance.mockReset().mockReturnValue(true);
});

it("sends an incomplete station state before the complete state", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.constructionPreviewReason.mockReturnValue("This coaster model cannot use this piece.");
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  postMessage.mockClear();
  onMessage({ data: { type: "construction-start", x: 5, y: 7, z: 8, heading: 2 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state").map(([message]) => message.complete)).toEqual([false, true]);
  const constructionStates = postMessage.mock.calls
    .map(([message]) => message)
    .filter((message): message is ConstructionState => message.type === "construction-state");
  expect(constructionStates.map((message) => message.appendReason)).toEqual([
    "This coaster model cannot use this piece.",
    "This coaster model cannot use this piece.",
  ]);
  expect(wasmMock.constructionStart).toHaveBeenCalledWith(5, 7, 8, 2);
  expect(wasmMock.builderEstimate.mock.calls.map(([complete]) => complete)).toEqual([false, true]);
  const sent = postMessage.mock.calls.map(([message]) => message);
  expect(sent.findIndex((message) => message.type === "world")).toBeLessThan(sent.findIndex((message) => message.type === "construction-state" && message.complete));
});

it("sends one complete placement state before the finish search result", async () => {
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
  await vi.advanceTimersByTimeAsync(0);

  postMessage.mockClear();
  onMessage({ data: { type: "construction-append" } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state").map(([message]) => message.complete)).toEqual([false, true]);
  onMessage({ data: { type: "construction-finish-await" } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.some(([message]) => message.type === "construction-finish-ready")).toBe(false);

  await vi.advanceTimersByTimeAsync(0);
  const messages = postMessage.mock.calls.map(([message]) => message);
  expect(messages.filter((message) => message.type === "construction-state").map((message) => message.complete)).toEqual([false, true]);
  expect(messages.filter((message) => message.type === "construction-finish-status")).toEqual([{
    type: "construction-finish-status",
    canFinish: false,
    finishReason: "Checking for a connection to the station.",
  }]);
  expect(messages.findIndex((message) => message.type === "construction-finish-status")).toBeLessThan(messages.findIndex((message) => message.type === "construction-finish-ready"));
});

it("dispatches saved shaped geometry through the exact resolved append bridge", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();

  const intentJson = '{"hill":{"feel":"Floater","strength_g":-0.5,"size":"Auto"}}';
  const specJson = '{"kind":"hill","design_speed_mps":34,"cars":8,"target_g":-0.5,"delivered_g":-0.5,"height_m":36,"span_m":52,"profile":{"points":[],"cached_minimum_entry":null,"cached_minimum_radius":null}}';
  postMessage.mockClear();
  onMessage({ data: { type: "construction-append-resolved", intentJson, specJson } } as MessageEvent);
  await Promise.resolve();

  expect(wasmMock.constructionAppendResolved).toHaveBeenCalledExactlyOnceWith(intentJson, specJson);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state").map(([message]) => message.complete)).toEqual([false, true]);
  expect(postMessage.mock.calls.some(([message]) => message.type === "world")).toBe(true);
});

it("publishes finish status for inactive construction and keeps a no-search await immediate", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();

  onMessage({ data: { type: "construction-finish-await" } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.map(([message]) => message.type)).toContain("construction-finish-ready");
  expect(postMessage.mock.calls.some(([message]) => message.type === "construction-finish-status")).toBe(false);

  postMessage.mockClear();
  onMessage({ data: { type: "facility-place", kind: "carousel", x: 0, y: 0, rotation: 0 } } as MessageEvent);
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(0);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state")).toHaveLength(0);
  expect(postMessage.mock.calls.find(([message]) => message.type === "construction-finish-status")?.[0]).toEqual({
    type: "construction-finish-status",
    canFinish: false,
    finishReason: "Checking for a connection to the station.",
  });
});

it("publishes construction state for facility edits before resuming the finish search", async () => {
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
  await vi.advanceTimersByTimeAsync(0);

  postMessage.mockClear();
  onMessage({ data: { type: "facility-place", kind: "carousel", x: 0, y: 0, rotation: 0 } } as MessageEvent);
  await Promise.resolve();
  const immediateStates = postMessage.mock.calls.filter(([message]) => message.type === "construction-state");
  expect(immediateStates).toHaveLength(1);
  expect(immediateStates[0][0].finishReason).toBe("Checking for a connection to the station.");

  await vi.advanceTimersByTimeAsync(0);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state")).toHaveLength(1);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-finish-status")).toHaveLength(1);

  postMessage.mockClear();
  onMessage({ data: { type: "facility-remove", id: 1 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state")).toHaveLength(1);
});

it("searches again after loading a park, because saves do not keep finish results", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "load-park", requestId: 1, bytes: new Uint8Array() } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.finishSearchStart).not.toHaveBeenCalled();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state")).toHaveLength(1);

  await vi.advanceTimersByTimeAsync(0);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(1);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state")).toHaveLength(1);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-finish-status")).toHaveLength(1);
});

it("keeps finish preview as a full state and publishes completion status after closure", async () => {
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
  await vi.advanceTimersByTimeAsync(0);

  postMessage.mockClear();
  onMessage({ data: { type: "construction-finish-preview" } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "construction-state")).toHaveLength(1);
  expect(postMessage.mock.calls.some(([message]) => message.type === "construction-finish-status")).toBe(false);

  wasmMock.finishConfirmCloses = true;
  postMessage.mockClear();
  onMessage({ data: { type: "construction-finish-confirm" } } as MessageEvent);
  await Promise.resolve();
  const completedState = postMessage.mock.calls.find(([message]) => message.type === "construction-state")?.[0];
  expect(completedState).toEqual(expect.objectContaining({ type: "construction-state", active: false }));
  onMessage({ data: { type: "construction-finish-await" } } as MessageEvent);
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(0);

  const messages = postMessage.mock.calls.map(([message]) => message);
  expect(messages.some((message) => message.type === "construction-finish-status")).toBe(true);
  expect(messages.findIndex((message) => message.type === "construction-finish-status")).toBeLessThan(messages.findIndex((message) => message.type === "construction-finish-ready"));
});

it("keeps replay awaiting its full command state and finish-ready barrier", async () => {
  class FakeWorker extends EventTarget {
    readonly sent: string[] = [];
    postMessage(action: { type: string }): void {
      this.sent.push(action.type);
      if (action.type === "construction-append") {
        this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-finish-status", canFinish: true, finishReason: "" } }));
      } else if (action.type === "construction-finish-await") {
        this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-finish-status", canFinish: false, finishReason: "Checking" } }));
        this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-finish-ready" } }));
      } else if (action.type === "construction-finish-preview") {
        this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-state", complete: true } }));
      } else if (action.type === "finish") {
        this.dispatchEvent(new MessageEvent("message", { data: { type: "finished" } }));
      }
    }
  }
  const worker = new FakeWorker();
  const session: ReplaySession = {
    format: "theme-park-session",
    version: 1,
    seed: "1",
    park: "new",
    guestCount: 0,
    commands: [
      { tick: 0, command: { type: "construction-append" } },
      { tick: 0, command: { type: "construction-finish-preview" } },
    ],
    checkpoints: [],
    finalTick: 0,
  };
  const replay = replaySessionIntoWorker(worker as unknown as Worker, session, { action: { type: "finish" }, replyType: "finished" });
  await Promise.resolve();
  expect(worker.sent).toEqual(["construction-append"]);
  worker.dispatchEvent(new MessageEvent("message", { data: { type: "construction-state", complete: true } }));
  await replay;
  expect(worker.sent).toEqual([
    "construction-append",
    "construction-finish-await",
    "construction-finish-preview",
    "finish",
  ]);
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

it("includes every coaster's train data in snapshots without construction-specific arrays", async () => {
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
  const snapshot = postMessage.mock.calls.find(([message]) => message.type === "snapshot")?.[0];
  expect(snapshot.coasters).toEqual([{ id: 0, trainPoses: new Float32Array(), trainStates: new Float32Array(), riders: new Uint32Array() }]);
});

it("includes per-train states in snapshots and transfers their buffers", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const coasterId = 17;
  const trainStates = new Float32Array([0, 8, 1, 1.2, 0.4, 1]);
  wasmMock.coasterIds.mockReturnValue(Uint32Array.of(0, coasterId));
  wasmMock.coasterTrainStates.mockImplementation((id: number) => id === coasterId ? trainStates : new Float32Array());
  wasmMock.coasterTrainPoses.mockImplementation((id: number) => id === coasterId ? Float32Array.of(1, 2) : new Float32Array());
  const riders = new Uint32Array([0, 3, 42]);
  wasmMock.coasterRiders.mockImplementation((id: number) => id === coasterId ? riders : new Uint32Array());
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });

  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  await Promise.resolve();

  const [snapshot, transfers] = postMessage.mock.calls.find(([message]) => message.type === "snapshot")!;
  expect(snapshot.coasters[1]).toEqual({ id: coasterId, trainPoses: Float32Array.of(1, 2), trainStates, riders });
  expect(transfers).toContain(trainStates.buffer);
  expect(transfers).toContain(riders.buffer);
});

it("resends track buffers only for coasters whose track revision changed", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const starterCoasterId = 0;
  const secondId = 17;
  const starter = { id: starterCoasterId, trackRevision: "starter:1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } };
  const second = { id: secondId, trackRevision: "park:1", colors: { track: "teal", supports: "cream", trains: "plum" }, trainLayout: { model: "velocity_viper", trackMaterial: "steel", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } };
  wasmMock.coasterRenderList.mockReturnValue(JSON.stringify([starter, second]));
  wasmMock.coasterIds.mockReturnValue(Uint32Array.of(starterCoasterId, secondId));
  wasmMock.editingCoasterId.mockReturnValue(secondId);
  wasmMock.pathBuild.mockReturnValue(true);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });

  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  const initialWorld = postMessage.mock.calls.find(([message]) => message.type === "world")?.[0] as CoasterWorldMessage;
  expect(initialWorld.editingCoasterId).toBe(secondId);
  expect(initialWorld.coasters.map(({ id }) => id)).toEqual([starterCoasterId, secondId]);
  expect(initialWorld.coasters.map(({ geometry }) => geometry instanceof Float32Array)).toEqual([true, true]);
  expect(wasmMock.coasterGeometry.mock.calls).toEqual([[starterCoasterId], [secondId]]);
  expect(wasmMock.coasterSupports.mock.calls).toEqual([[starterCoasterId], [secondId]]);

  const buildPath = { type: "path-build", requestId: 1, kind: "normal", style: 0, start: { x: 0, y: 0, z: 0 }, end: { x: 1, y: 0, z: 0 } } as const;
  onMessage({ data: buildPath } as MessageEvent);
  await Promise.resolve();
  const unchangedWorld = postMessage.mock.calls.findLast(([message]) => message.type === "world")?.[0] as CoasterWorldMessage;
  expect(unchangedWorld.coasters.map(({ id }) => id)).toEqual([starterCoasterId, secondId]);
  expect(unchangedWorld.coasters.every((coaster) => !("geometry" in coaster) && !("rails" in coaster) && !("ties" in coaster) && !("supports" in coaster))).toBe(true);
  expect(wasmMock.coasterGeometry.mock.calls).toEqual([[starterCoasterId], [secondId]]);
  expect(wasmMock.coasterSupports.mock.calls).toEqual([[starterCoasterId], [secondId]]);

  wasmMock.coasterRenderList.mockReturnValue(JSON.stringify([starter, { ...second, trackRevision: "park:2" }]));
  onMessage({ data: { ...buildPath, requestId: 2 } } as MessageEvent);
  await Promise.resolve();
  const changedWorld = postMessage.mock.calls.findLast(([message]) => message.type === "world")?.[0] as CoasterWorldMessage;
  expect(changedWorld.coasters[0]).not.toHaveProperty("geometry");
  expect(changedWorld.coasters[1]).toMatchObject({ id: secondId, trackRevision: "park:2", geometry: Float32Array.of(secondId) });
  expect(wasmMock.coasterGeometry.mock.calls).toEqual([[starterCoasterId], [secondId], [secondId]]);
  expect(wasmMock.coasterSupports.mock.calls).toEqual([[starterCoasterId], [secondId], [secondId]]);
});

it("routes ride information and tests through numeric coaster ID zero", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ id: 0, name: "Daisy Dash", model: "velocity_viper", open: false, price: 0, income: 0, queueLength: 0, complete: true }]));
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "new", guestCount: 0, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  onMessage({ data: { type: "ride-info-request", ride: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.rideInfo).toHaveBeenCalledExactlyOnceWith(0);
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "ride-info")?.[0]).toMatchObject({
    type: "ride-info",
    info: { ride: 0 },
  });

  onMessage({ data: { type: "ride-test", ride: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.rideTest).toHaveBeenCalledExactlyOnceWith(0);
  expect(wasmMock.rideInfo).toHaveBeenCalledTimes(2);
});

it("returns captured queue entry places after a test tick advance", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.testTakeQueueEntryPlacesJson.mockReturnValue("[101,202]");
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { name: "theme-park-test-manual", postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "new", guestCount: 2, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();

  onMessage({ data: { type: "test-advance", count: 2 } } as MessageEvent);
  await Promise.resolve();

  expect(wasmMock.testBeginQueueEntryCapture).toHaveBeenCalledOnce();
  expect(wasmMock.tick).toHaveBeenCalledTimes(2);
  expect(wasmMock.testTakeQueueEntryPlacesJson).toHaveBeenCalledOnce();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "test-advanced")?.[0]).toEqual({
    type: "test-advanced",
    queueEntryPlaceIds: [101, 202],
  });
});

it("publishes coaster selection, ride controls, edits, rename, and demolition through the worker", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  const coaster = { id: 0, name: "Daisy Dash", model: "velocity_viper", open: false, price: 10, income: 5, queueLength: 3, complete: true };
  wasmMock.coastersJson.mockReturnValue(JSON.stringify([coaster]));
  wasmMock.coasterRenderList.mockReturnValue(JSON.stringify([{ id: 0, trackRevision: "daisy:1", colors: null, trainLayout: { model: "velocity_viper", trackMaterial: "steel", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } }]));
  wasmMock.rideInfo.mockReturnValue(JSON.stringify({ ride: 0, modelName: "Velocity Viper", manufacturer: "Vekoma", limits: {}, report: null }));
  wasmMock.coasterSetOpen.mockReturnValue(true);
  wasmMock.coasterSetPrice.mockReturnValue(true);
  wasmMock.coasterRename.mockReturnValue(true);
  wasmMock.coasterEdit.mockReturnValue(true);
  wasmMock.coasterDemolish.mockReturnValue(true);

  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "new", guestCount: 0, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();

  expect(postMessage.mock.calls.some(([message]) => message.type === "coasters" && message.coasters[0].name === "Daisy Dash")).toBe(true);
  onMessage({ data: { type: "ride-info-request", ride: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "ride-info")?.[0].info).toMatchObject({
    ride: 0,
    name: "Daisy Dash",
    open: false,
    price: 10,
    income: 5,
    complete: true,
    editable: true,
  });

  const rideInfoCount = (): number => postMessage.mock.calls.filter(([message]) => message.type === "ride-info").length;
  const initialRideInfoCount = rideInfoCount();
  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ ...coaster, queueLength: 4 }]));
  onMessage({ data: { type: "test-advance", count: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "coasters")?.[0].coasters[0].queueLength).toBe(4);
  expect(rideInfoCount()).toBe(initialRideInfoCount);

  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ ...coaster, queueLength: 4, income: 6 }]));
  onMessage({ data: { type: "test-advance", count: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "ride-info")?.[0].info.income).toBe(6);
  expect(rideInfoCount()).toBe(initialRideInfoCount + 1);

  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ ...coaster, open: true }]));
  onMessage({ data: { type: "ride-set-open", ride: 0, open: true } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterSetOpen).toHaveBeenCalledExactlyOnceWith(0, true);
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "coaster-action-result")?.[0]).toMatchObject({ action: "open", coasterId: 0, success: true });
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "ride-info")?.[0].info.open).toBe(true);

  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ ...coaster, open: true, price: 12 }]));
  onMessage({ data: { type: "ride-set-price", ride: 0, price: 12 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterSetPrice).toHaveBeenCalledExactlyOnceWith(0, 12);

  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ ...coaster, name: "Daisy Dash 2" }]));
  onMessage({ data: { type: "ride-rename", ride: 0, name: "Daisy Dash 2" } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterRename).toHaveBeenCalledExactlyOnceWith(0, "Daisy Dash 2");
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "coasters")?.[0].coasters[0].name).toBe("Daisy Dash 2");

  wasmMock.editingCoasterId.mockReturnValue(0);
  wasmMock.constructionActive.mockReturnValue(true);
  onMessage({ data: { type: "ride-edit", ride: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterEdit).toHaveBeenCalledExactlyOnceWith(0);
  expect(postMessage.mock.calls.some(([message]) => message.type === "world" && message.editingCoasterId === 0)).toBe(true);

  wasmMock.editingCoasterId.mockReturnValue(null);
  wasmMock.constructionActive.mockReturnValue(false);
  onMessage({ data: { type: "ride-stop-editing" } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterStopEditing).toHaveBeenCalledOnce();

  wasmMock.coastersJson.mockReturnValue("[]");
  wasmMock.coasterRenderList.mockReturnValue("[]");
  onMessage({ data: { type: "ride-demolish", ride: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterDemolish).toHaveBeenCalledExactlyOnceWith(0);
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "coaster-action-result")?.[0]).toMatchObject({ action: "demolish", coasterId: 0, success: true });
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "coasters")?.[0].coasters).toEqual([]);

  wasmMock.coasterIds.mockReturnValue(new Uint32Array());
  onMessage({ data: { type: "test-advance", count: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "snapshot")?.[0].coasters).toEqual([]);
});

it("uses numeric IDs for the starter coaster and the coaster being built", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const editingCoasterId = 42;
  const starterCoasterId = 0;
  wasmMock.editingCoasterId.mockReturnValue(editingCoasterId);
  const starterCoaster = { id: starterCoasterId, name: "Timber Tumbler", model: "hybrid", open: false, price: 0, income: 0, queueLength: 0, complete: true };
  const editingCoaster = { id: editingCoasterId, name: "Daisy Dash", model: "velocity_viper", open: false, price: 0, income: 0, queueLength: 0, complete: true };
  wasmMock.coastersJson.mockReturnValue(JSON.stringify([starterCoaster, editingCoaster]));
  wasmMock.coasterSetOpen.mockReturnValue(true);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "new", guestCount: 0, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();

  onMessage({ data: { type: "ride-test", ride: editingCoasterId } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.rideTest).toHaveBeenCalledExactlyOnceWith(editingCoasterId);
  expect(wasmMock.rideInfo).toHaveBeenCalledExactlyOnceWith(editingCoasterId);

  onMessage({ data: { type: "ride-info-request", ride: starterCoasterId } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.rideInfo).toHaveBeenLastCalledWith(starterCoasterId);
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "ride-info")?.[0].info).toMatchObject({ name: "Timber Tumbler", editable: true, open: false, price: 0 });

  wasmMock.coastersJson.mockReturnValue(JSON.stringify([{ ...starterCoaster, open: true }, editingCoaster]));
  onMessage({ data: { type: "ride-set-open", ride: starterCoasterId, open: true } } as MessageEvent);
  await Promise.resolve();
  expect(wasmMock.coasterSetOpen).toHaveBeenCalledExactlyOnceWith(starterCoasterId, true);
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "coaster-action-result")?.[0]).toMatchObject({ action: "open", coasterId: starterCoasterId, success: true });

  onMessage({ data: { type: "export-session", requestId: 1 } } as MessageEvent);
  await Promise.resolve();
  const recording = postMessage.mock.calls.findLast(([message]) => message.type === "session-export")?.[0].recording;
  expect(recording.commands).toEqual([
    { tick: 0, command: { type: "ride-test", ride: editingCoasterId } },
    { tick: 0, command: { type: "ride-set-open", ride: starterCoasterId, open: true } },
  ]);
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
  const latestFinishStatus = (): Extract<WorkerMessage, { type: "construction-finish-status" }> | undefined => postMessage.mock.calls
    .map(([message]) => message as WorkerMessage)
    .filter((message): message is Extract<WorkerMessage, { type: "construction-finish-status" }> => message.type === "construction-finish-status")
    .at(-1);

  await dispatchAndFlush({ type: "construction-start", x: 0, y: 0, z: 0, heading: 0 });
  expect(latestConstructionState()?.canFinish).toBe(false);
  expect(latestFinishStatus()?.canFinish).toBe(true);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(1);

  await dispatchAndFlush({ type: "construction-select", piece: "Straight" });
  expect(latestConstructionState()?.canFinish).toBe(true);
  expect(latestFinishStatus()?.canFinish).toBe(true);
  expect(wasmMock.finishInvalidate).toHaveBeenCalledTimes(1);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(1);

  const trackEdits: WorkerAction[] = [
    { type: "construction-append" },
    { type: "construction-undo" },
    { type: "construction-delete", id: 0 },
  ];
  for (const action of trackEdits) {
    await dispatchAndFlush(action);
    expect(latestConstructionState()?.canFinish).toBe(false);
    expect(latestFinishStatus()?.canFinish).toBe(true);
  }
  expect(wasmMock.finishInvalidate).toHaveBeenCalledTimes(4);
  expect(wasmMock.finishSearchStart).toHaveBeenCalledTimes(4);
  expect(postMessage.mock.calls.some(([message]) => message.type === "construction-finish-ready")).toBe(false);
});

it("sends coaster colours to the simulation and publishes them parsed, with the demo coaster's in the first world", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();
  const world = postMessage.mock.calls.find(([message]) => message.type === "world")?.[0] as CoasterWorldMessage;
  expect(world.coasters[0].colors).toEqual({ track: "coral", supports: "timber", trains: "sage" });
  expect(world.coasters[0].trainLayout).toEqual({ model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });
  onMessage({ data: { type: "construction-start", x: 0, y: 0, z: 0, heading: 0 } } as MessageEvent);
  await Promise.resolve();
  const construction = postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0];
  expect(construction.colors).toEqual({ track: "coral", supports: "timber", trains: "sage" });
  expect(construction.trainLayout).toEqual({ model: "velocity_viper", trackMaterial: "steel", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });
  onMessage({ data: { type: "construction-set-colors", colors: { track: "teal", supports: "cream", trains: "plum" } } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.findLast(([message]) => message.type === "construction-state")?.[0].colors).toEqual({ track: "teal", supports: "cream", trains: "plum" });
});

it("publishes coaster color and train layout changes to the world", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.constructionActive.mockReturnValue(true);
  wasmMock.editingCoasterId.mockReturnValue(1);
  const starter = { id: 0, trackRevision: "starter:1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } };
  const built = { id: 1, trackRevision: "built:1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "velocity_viper", trackMaterial: "steel", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } };
  wasmMock.coasterRenderList.mockReturnValue(JSON.stringify([starter, built]));
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  onMessage({ data: { type: "init", seed: 2026n, park: "new", guestCount: 0, frozen: true, manualBlockAnalysis: true } } as MessageEvent);
  await Promise.resolve();

  postMessage.mockClear();
  const repainted = { ...built, trackRevision: "built:2", colors: { track: "teal", supports: "cream", trains: "plum" } };
  wasmMock.coasterRenderList.mockReturnValue(JSON.stringify([starter, repainted]));
  onMessage({ data: { type: "construction-set-colors", colors: repainted.colors } } as MessageEvent);
  await Promise.resolve();
  const colorWorld = postMessage.mock.calls.findLast(([message]) => message.type === "world")?.[0] as CoasterWorldMessage | undefined;
  expect(colorWorld?.coasters.find(({ id }) => id === 1)?.colors).toEqual(repainted.colors);

  postMessage.mockClear();
  const changedTrain = { ...repainted, trackRevision: "built:3", trainLayout: { ...repainted.trainLayout, carsPerTrain: 4, lengthMetres: 12 } };
  wasmMock.coasterRenderList.mockReturnValue(JSON.stringify([starter, changedTrain]));
  onMessage({ data: { type: "construction-set-cars-per-train", cars: 4 } } as MessageEvent);
  await Promise.resolve();
  const trainWorld = postMessage.mock.calls.findLast(([message]) => message.type === "world")?.[0] as CoasterWorldMessage | undefined;
  expect(trainWorld?.coasters.find(({ id }) => id === 1)?.trainLayout).toEqual(changedTrain.trainLayout);
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

it("publishes the bridge error when a park description fails to initialize", async () => {
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.fromParkDescription.mockImplementation(() => {
    throw new Error("paths[2] segment 0: Path is blocked");
  });
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");

  onMessage({ data: {
    type: "init",
    seed: 2026n,
    park: "description",
    descriptionJson: "{}",
    guestCount: 12,
    frozen: true,
    manualBlockAnalysis: false,
  } } as MessageEvent);
  await Promise.resolve();
  await Promise.resolve();

  expect(postMessage.mock.calls.map(([message]) => message)).toContainEqual({
    type: "initialization-error",
    error: "paths[2] segment 0: Path is blocked",
  });
});
