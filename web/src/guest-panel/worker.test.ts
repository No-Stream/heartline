import { afterEach, expect, it, vi } from "vitest";

const wasmMock = vi.hoisted(() => ({ initialize: vi.fn(), tick: vi.fn() }));

vi.mock("../wasm/theme_park_sim_wasm.js", () => ({
  default: wasmMock.initialize,
  WasmSimulation: class {
    world_dimensions() { return new Uint16Array([3, 3]); }
    world_surfaces() { return new Int32Array(54); }
    world_paths() { return new Int32Array(); }
    world_carved() { return new Int32Array(); }
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
