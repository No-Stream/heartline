import { afterEach, describe, expect, it, vi } from "vitest";
import { captureWorldSnapshot, validateSurfaceEdit } from "./world-snapshot";

const workerMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  tick: vi.fn(),
  setSurface: vi.fn(),
  pieceShapes: vi.fn(() => "[]"),
  builderStart: vi.fn(),
  builderSelect: vi.fn(),
  builderEstimate: vi.fn(),
  coasterRenderList: vi.fn(() => JSON.stringify([{ id: 4294967295, trackRevision: "demo:1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } }])),
  coasterIds: vi.fn(() => Uint32Array.of(4294967295)),
  editingCoasterId: vi.fn(() => null as number | null),
  coasterGeometry: vi.fn((_id: number) => new Float32Array()),
  coasterRails: vi.fn((_id: number) => new Float32Array([1, 2])),
  coasterTies: vi.fn((_id: number) => new Float32Array([3])),
  coasterSupports: vi.fn((_id: number) => new Float32Array()),
  coasterTrainPoses: vi.fn((_id: number) => new Float32Array()),
  coasterTrainStates: vi.fn((_id: number) => new Float32Array([0, 4, 0, 1.1, 0.2, 1])),
  coasterRiders: vi.fn((_id: number) => new Uint32Array()),
  coastersJson: vi.fn(() => "[]"),
}));

vi.mock("./wasm/theme_park_sim_wasm.js", () => ({
  default: workerMock.initialize,
  piece_shapes_json: workerMock.pieceShapes,
  BuilderSessionBridge: class {
    invalidate() {}
    start(_simulation: object, x: number, y: number, z: number, heading: number) {
      workerMock.builderStart(x, y, z, heading);
    }
    select(_simulation: object, piece: string, exact: boolean) {
      workerMock.builderSelect(piece, exact);
      return "";
    }
    estimates(_simulation: object, complete: boolean) {
      workerMock.builderEstimate(complete);
      return JSON.stringify({ constructionForcePreview: null, pieces: [], elements: [], hillOptions: [], speedTurns: [], candidateForcePreview: null, selectedShape: null, speedMatch: null });
    }
    free() {}
  },
  WasmSimulation: class {
    scenery_json() { return "[]"; }
    facilities_json() { return "[]"; }
    facility_revenue() { return 0n; }
    world_dimensions() { return new Uint16Array([3, 3]); }
    world_surfaces() { return new Int32Array(54); }
    world_paths() { return new Int32Array(); }
    world_path_metadata() { return new Int32Array(); }
    world_facility_openings() { return new Int32Array(); }
    world_carved() { return new Int32Array(); }
    tick() { workerMock.tick(); }
    tick_count() { return BigInt(workerMock.tick.mock.calls.length); }
    guest_ids() { return new Uint32Array([7]); }
    snapshot() { return new Float32Array([1, 2, 3, 1, 0, 4, 1]); }
    guest_appearances() { return new Uint8Array([1, 2, 3, 4, 0]); }
    coaster_render_list_json() { return workerMock.coasterRenderList(); }
    coaster_ids() { return workerMock.coasterIds(); }
    editing_coaster_id_value() { return workerMock.editingCoasterId(); }
    coaster_geometry(id: number) { workerMock.coasterGeometry(id); return new Float32Array(); }
    coaster_rails(id: number) { workerMock.coasterRails(id); return new Float32Array([1, 2]); }
    coaster_ties(id: number) { workerMock.coasterTies(id); return new Float32Array([3]); }
    coaster_supports(id: number) { workerMock.coasterSupports(id); return new Float32Array(); }
    coaster_train_poses(id: number) { workerMock.coasterTrainPoses(id); return new Float32Array(); }
    coaster_train_states(id: number) { return workerMock.coasterTrainStates(id); }
    coaster_riders(id: number) { return workerMock.coasterRiders(id); }
    coasters_json() { return workerMock.coastersJson(); }
    construction_message() { return ""; }
    construction_coaster_price() { return 0; }
    construction_coaster_income() { return 0; }
    construction_active() { return false; }
    construction_finish_preview_active() { return false; }
    construction_can_finish() { return false; }
    construction_finish_reason() { return ""; }
    construction_finish_search_start() {}
    construction_finish_search_advance() { return true; }
    construction_finish_invalidate() {}
    construction_finish_preview() {}
    construction_finish_confirm() {}
    construction_finish_cancel() {}
    set_world_surface(x: number, y: number, corners: Int16Array, water: number | undefined) {
      return workerMock.setSurface(x, y, corners, water);
    }
  },
}));

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.resetModules();
  workerMock.tick.mockReset();
  workerMock.initialize.mockReset();
  workerMock.setSurface.mockReset();
  workerMock.builderStart.mockReset();
  workerMock.builderSelect.mockReset();
  workerMock.builderEstimate.mockReset();
  workerMock.coasterRenderList.mockReset().mockReturnValue(JSON.stringify([{ id: 4294967295, trackRevision: "demo:1", colors: { track: "coral", supports: "timber", trains: "sage" }, trainLayout: { model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 } }]));
  workerMock.coasterIds.mockReset().mockReturnValue(Uint32Array.of(4294967295));
  workerMock.editingCoasterId.mockReset().mockReturnValue(null);
  workerMock.coasterGeometry.mockReset();
  workerMock.coasterRails.mockReset();
  workerMock.coasterTies.mockReset();
  workerMock.coasterSupports.mockReset();
  workerMock.coasterTrainPoses.mockReset();
  workerMock.coasterTrainStates.mockReset().mockReturnValue(new Float32Array([0, 4, 0, 1.1, 0.2, 1]));
  workerMock.coasterRiders.mockReset();
  workerMock.coastersJson.mockReset().mockReturnValue("[]");
});

describe("world snapshot transfer", () => {
  it("captures each world array and transfers its buffer once", () => {
    const dimensions = new Uint16Array([2, 1]);
    const surfaces = new Int32Array(12);
    const paths = new Int32Array([1, 0, 4, 0]);
    const carved = new Int32Array([1, 0, 0, 4]);
    const pathMetadata = new Int32Array([1, 2, 0]);
    const facilityOpenings = new Int32Array([0, 0, 0, 2]);
    const result = captureWorldSnapshot({
      world_dimensions: () => dimensions,
      world_surfaces: () => surfaces,
      world_paths: () => paths,
      world_path_metadata: () => pathMetadata,
      world_facility_openings: () => facilityOpenings,
      world_carved: () => carved,
    });
    expect(result.message).toEqual({ type: "world", dimensions, surfaces, paths, pathMetadata, facilityOpenings, carved });
    expect(result.transfer).toEqual([dimensions.buffer, surfaces.buffer, paths.buffer, pathMetadata.buffer, facilityOpenings.buffer, carved.buffer]);
  });
});

it("rejects terrain edits that cannot be represented in the world", () => {
  const valid = { type: "set-surface" as const, x: 1, y: 0, corners: new Int16Array([1, 2, 3, 4]), water: -1 };
  const dimensions = new Uint16Array([2, 1]);
  expect(() => validateSurfaceEdit(valid, dimensions)).not.toThrow();
  for (const edit of [
    { ...valid, x: 2 }, { ...valid, x: 0.5 }, { ...valid, y: -1 },
    { ...valid, corners: new Int16Array([1, 2, 3]) },
    { ...valid, water: 32768 }, { ...valid, water: 0.5 },
  ]) expect(() => validateSurfaceEdit(edit, dimensions)).toThrow();
});

it("starts simulation time after WebAssembly initialization completes", async () => {
  vi.useFakeTimers();
  let finishInitialization!: () => void;
  workerMock.initialize.mockReturnValue(new Promise<void>((resolve) => { finishInitialization = resolve; }));
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("./worker");
  onMessage({ data: { type: "init", seed: 42n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);

  vi.advanceTimersByTime(1_000);
  finishInitialization();
  await Promise.resolve();

  expect(postMessage).toHaveBeenCalledTimes(7);
  const [world, worldTransfer] = postMessage.mock.calls[0];
  expect(world.coasters[0].rails).toEqual(new Float32Array([1, 2]));
  expect(world.coasters[0].ties).toEqual(new Float32Array([3]));
  expect(world.coasters[0].trainLayout).toEqual({ model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });
  expect(worldTransfer).toContain(world.coasters[0].rails.buffer);
  expect(worldTransfer).toContain(world.coasters[0].ties.buffer);
  expect(workerMock.tick).not.toHaveBeenCalled();
  expect(postMessage.mock.calls[1][0]).toEqual({ type: "coasters", coasters: [{
    id: 4294967295,
    name: "Timber Tumbler",
    model: "hybrid",
    open: false,
    price: 0,
    income: 0,
    queueLength: 0,
    complete: true,
  }] });
  expect(postMessage.mock.calls[2][0]).toEqual({ type: "facilities", facilities: [], message: "", revenue: "0", placed: false, forced: false, outcome: null });
  expect(postMessage.mock.calls[3][0]).toEqual({ type: "scenery", scenery: [], message: "", outcome: null });
  expect(postMessage.mock.calls[4][0]).toEqual({ type: "piece-shapes", shapes: [] });
  expect(postMessage.mock.calls[5][0]).toEqual({ type: "ready" });
  expect(postMessage.mock.calls[6][0]).toMatchObject({ type: "snapshot", tickCount: 0 });
  const [snapshot, transfer] = postMessage.mock.calls[6];
  expect(snapshot.guestIds).toEqual(new Uint32Array([7]));
  expect(snapshot.guests).toEqual(new Float32Array([1, 2, 3, 1, 0, 4, 1]));
  expect(snapshot.appearances).toEqual(new Uint8Array([1, 2, 3, 4, 0]));
  expect(snapshot.coasters[0].trainStates).toEqual(new Float32Array([0, 4, 0, 1.1, 0.2, 1]));
  expect(transfer).toEqual([
    snapshot.guestIds.buffer, snapshot.guests.buffer, snapshot.appearances.buffer,
    snapshot.coasters[0].trainPoses.buffer, snapshot.coasters[0].trainStates.buffer, snapshot.coasters[0].riders.buffer,
  ]);
  vi.advanceTimersByTime(34);
  expect(workerMock.tick).toHaveBeenCalledTimes(1);
});

it("publishes an updated world only after a successful terrain change", async () => {
  vi.useFakeTimers();
  workerMock.initialize.mockResolvedValue(undefined);
  workerMock.setSurface.mockReturnValueOnce(true).mockReturnValueOnce(false);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("./worker");
  onMessage({ data: { type: "init", seed: 42n, park: "demo", guestCount: 12, frozen: false, manualBlockAnalysis: false } } as MessageEvent);
  await Promise.resolve();
  const edit = { type: "set-surface", x: 1, y: 2, corners: new Int16Array([3, 4, 5, 6]), water: 7 };
  onMessage({ data: edit } as MessageEvent);
  await Promise.resolve();
  expect(workerMock.setSurface).toHaveBeenCalledWith(1, 2, edit.corners, 7);
  expect(postMessage.mock.calls.filter(([message]) => message.type === "world")).toHaveLength(2);
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "surface-edited", changed: true });
  onMessage({ data: edit } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.filter(([message]) => message.type === "world")).toHaveLength(2);
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "surface-edited", changed: false });
});
