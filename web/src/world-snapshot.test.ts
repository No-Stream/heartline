import { afterEach, describe, expect, it, vi } from "vitest";
import { captureWorldSnapshot, validateSurfaceEdit } from "./world-snapshot";

const workerMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  tick: vi.fn(),
  setSurface: vi.fn(),
}));

vi.mock("./wasm/theme_park_sim_wasm.js", () => ({
  default: workerMock.initialize,
  WasmSimulation: class {
    world_dimensions() { return new Uint16Array([3, 3]); }
    world_surfaces() { return new Int32Array(54); }
    world_paths() { return new Int32Array(); }
    world_carved() { return new Int32Array(); }
    tick() { workerMock.tick(); }
    tick_count() { return BigInt(workerMock.tick.mock.calls.length); }
    snapshot() { return new Float32Array(); }
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
});

describe("world snapshot transfer", () => {
  it("captures each world array and transfers its buffer once", () => {
    const dimensions = new Uint16Array([2, 1]);
    const surfaces = new Int32Array(12);
    const paths = new Int32Array([1, 0, 4, 0]);
    const carved = new Int32Array([1, 0, 0, 4]);
    const result = captureWorldSnapshot({
      world_dimensions: () => dimensions,
      world_surfaces: () => surfaces,
      world_paths: () => paths,
      world_carved: () => carved,
    });
    expect(result.message).toEqual({ type: "world", dimensions, surfaces, paths, carved });
    expect(result.transfer).toEqual([dimensions.buffer, surfaces.buffer, paths.buffer, carved.buffer]);
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
  vi.stubGlobal("self", { postMessage, addEventListener: vi.fn() });
  await import("./worker");

  vi.advanceTimersByTime(1_000);
  finishInitialization();
  await Promise.resolve();

  expect(postMessage).toHaveBeenCalledTimes(3);
  expect(workerMock.tick).not.toHaveBeenCalled();
  expect(postMessage.mock.calls[2][0]).toMatchObject({ type: "snapshot", tickCount: 0 });
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
