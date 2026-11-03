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
let coasterGeometrySent = false;

function wake(): void {
  const now = performance.now();
  const result = advanceSimulation(clock, now - lastWakeup);
  lastWakeup = now;
  clock = result.clock;
  for (let tick = 0; tick < result.ticksToRun; tick += 1) simulation.tick();

  const guestIds = simulation.guest_ids();
  const guests = simulation.snapshot();
  const appearances = simulation.guest_appearances();
  const trainPoses = simulation.coaster_train_poses();
  workerScope.postMessage(
    { type: "snapshot", tickCount: Number(simulation.tick_count()), guestIds, guests, appearances, trainPoses },
    [guestIds.buffer, guests.buffer, appearances.buffer, trainPoses.buffer],
  );
  setTimeout(wake, Math.max(0, clock.tickDurationMs - clock.accumulatorMs));
}

function sendWorld(): void {
  const world = captureWorldSnapshot(simulation);
  const coasterSupports = simulation.coaster_supports();
  if (!coasterGeometrySent) {
    const coasterGeometry = simulation.coaster_geometry();
    const coasterRails = simulation.coaster_rails();
    const coasterTies = simulation.coaster_ties();
    workerScope.postMessage({ ...world.message, coasterGeometry, coasterRails, coasterTies, coasterSupports }, [...world.transfer, coasterGeometry.buffer, coasterRails.buffer, coasterTies.buffer, coasterSupports.buffer]);
    coasterGeometrySent = true;
  } else {
    workerScope.postMessage({ ...world.message, coasterSupports }, [...world.transfer, coasterSupports.buffer]);
  }
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
