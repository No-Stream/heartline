import init, { WasmSimulation } from "./wasm/theme_park_sim_wasm.js";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";

const tickRate = 30;
const workerScope = self as unknown as {
  postMessage(message: unknown, transfer?: Transferable[]): void;
};
let simulation: WasmSimulation;
let clock = createSimulationClock(tickRate, 4);
let lastWakeup = performance.now();

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

void init().then(() => {
  simulation = new WasmSimulation(2026n, 12);
  workerScope.postMessage({ type: "ready" });
  wake();
});
