import init, { WasmSimulation, WasmWalkingBenchmark } from '../src/wasm/theme_park_sim_wasm.js';

const guestCount = 10_000;
const warmupTicks = 20;
const measuredTicks = 200;

function measure(Simulation) {
  const simulation = new Simulation(42n, guestCount);
  for (let tick = 0; tick < warmupTicks; tick++) simulation.tick();
  const start = performance.now();
  for (let tick = 0; tick < measuredTicks; tick++) simulation.tick();
  const elapsedMs = performance.now() - start;
  const stateHash = simulation.state_hash().toString(16).padStart(16, '0');
  const snapshotBytes = simulation.guest_ids().byteLength + simulation.snapshot().byteLength + simulation.guest_appearances().byteLength;
  simulation.free();
  return { guestCount, warmupTicks, measuredTicks, elapsedMs, stateHash, snapshotBytes };
}

try {
  await init();
  self.postMessage({ directWalking: measure(WasmWalkingBenchmark), gameplay: measure(WasmSimulation) });
} catch (error) {
  self.postMessage({ error: String(error) });
}
