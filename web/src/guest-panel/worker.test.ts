import { afterEach, expect, it, vi } from "vitest";

const wasmMock = vi.hoisted(() => ({ initialize: vi.fn(), tick: vi.fn(), pieceShapes: vi.fn(() => "[]"), pathPreview: vi.fn(), pathBuild: vi.fn(), pathRemove: vi.fn() }));

vi.mock("../wasm/theme_park_sim_wasm.js", () => ({
  default: wasmMock.initialize,
  piece_shapes_json: wasmMock.pieceShapes,
  WasmSimulation: class {
    facilities_json() { return "[]"; }
    facility_revenue() { return 0n; }
    world_dimensions() { return new Uint16Array([3, 3]); }
    world_surfaces() { return new Int32Array(54); }
    world_paths() { return new Int32Array(); }
    world_path_metadata() { return new Int32Array(); }
    world_carved() { return new Int32Array(); }
    path_preview(coordinates: Int32Array, kind: string, style: number) { return wasmMock.pathPreview(coordinates, kind, style); }
    path_build(coordinates: Int32Array, kind: string, style: number) { return wasmMock.pathBuild(coordinates, kind, style); }
    path_remove(x: number, y: number, z: number) { return wasmMock.pathRemove(x, y, z); }
    path_message() { return "Path route does not connect across the terrain"; }
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
    coaster_train_poses() { return new Float32Array(); }
    construction_train_poses() { return new Float32Array(); }
    construction_message() { return ""; }
  },
}));

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.resetModules();
  wasmMock.tick.mockReset();
  wasmMock.initialize.mockReset();
  wasmMock.pathPreview.mockReset();
  wasmMock.pathBuild.mockReset();
  wasmMock.pathRemove.mockReset();
});

it("echoes the preview request id and reports invalid tiles", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.pathPreview.mockReturnValue(new Int32Array([0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 4]));
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
  await Promise.resolve();
  onMessage({ data: { type: "path-preview", requestId: 7, start: { x: 0, y: 0, z: 0 }, end: { x: 1, y: 0, z: 0 }, kind: "normal", style: 0 } } as MessageEvent);
  await Promise.resolve();
  expect(postMessage.mock.calls.at(-1)?.[0]).toEqual({ type: "path-preview", requestId: 7, valid: false, paths: expect.any(Int32Array), message: "Path route does not connect across the terrain" });
});

it("echoes path command request ids so the controller can ignore stale results", async () => {
  vi.useFakeTimers();
  wasmMock.initialize.mockResolvedValue(undefined);
  wasmMock.pathBuild.mockReturnValue(true);
  const postMessage = vi.fn();
  let onMessage!: (event: MessageEvent) => void;
  vi.stubGlobal("self", { postMessage, addEventListener: (_type: string, listener: (event: MessageEvent) => void) => { onMessage = listener; } });
  await import("../worker");
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
