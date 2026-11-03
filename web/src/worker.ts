import init, { WasmSimulation } from "./wasm/theme_park_sim_wasm.js";
import { captureWorldSnapshot, validateSurfaceEdit, type SetSurfaceMessage } from "./world-snapshot";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";

const tickRate = 30;
const workerScope = self as unknown as {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  addEventListener(type: "message", listener: (event: MessageEvent<SetSurfaceMessage>) => void): void;
};
let simulation: WasmSimulation;
let clock = createSimulationClock(tickRate, 4);
let lastWakeup: number;

function wake(): void {
  const now = performance.now();
  const result = advanceSimulation(clock, now - lastWakeup);
  lastWakeup = now;
  clock = result.clock;
  for (let tick = 0; tick < result.ticksToRun; tick += 1) simulation.tick();

  const positions = new Float32Array(simulation.snapshot());
  workerScope.postMessage(
    { type: "snapshot", tickCount: Number(simulation.tick_count()), positions },
    [positions.buffer],
  );
  setTimeout(wake, Math.max(0, clock.tickDurationMs - clock.accumulatorMs));
}

function sendWorld(): void {
  const world = captureWorldSnapshot(simulation);
  workerScope.postMessage(world.message, world.transfer);
}

const initialized = init().then(() => {
  simulation = new WasmSimulation(2026n, 12);
  sendWorld();
  workerScope.postMessage({ type: "ready" });
  lastWakeup = performance.now();
  wake();
});

workerScope.addEventListener("message", (event) => {
  void initialized.then(() => {
    validateSurfaceEdit(event.data, simulation.world_dimensions());
    const { x, y, corners, water } = event.data;
    const changed = simulation.set_world_surface(x, y, corners, water);
    if (changed) sendWorld();
    workerScope.postMessage({ type: "surface-edited", changed });
  });
});
