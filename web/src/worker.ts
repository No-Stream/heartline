import init, { WasmSimulation } from "./wasm/theme_park_sim_wasm.js";
import { captureWorldSnapshot, validateSurfaceEdit, type SetSurfaceMessage } from "./world-snapshot";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";
import type { ConstructionAction, ConstructionPiece } from "./construction-protocol";

const tickRate = 30;
const workerScope = self as unknown as {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  addEventListener(type: "message", listener: (event: MessageEvent<SetSurfaceMessage | ConstructionAction>) => void): void;
};
let simulation: WasmSimulation;
let clock = createSimulationClock(tickRate, 4);
let lastWakeup: number;
let coasterGeometrySent = false;
let lastConstructionMessage = "";

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
  const constructionTrainPoses = simulation.construction_train_poses();
  workerScope.postMessage(
    { type: "snapshot", tickCount: Number(simulation.tick_count()), guestIds, guests, appearances, trainPoses, constructionTrainPoses },
    [guestIds.buffer, guests.buffer, appearances.buffer, trainPoses.buffer, constructionTrainPoses.buffer],
  );
  const message = simulation.construction_message();
  if (message !== lastConstructionMessage) sendConstruction();
  setTimeout(wake, Math.max(0, clock.tickDurationMs - clock.accumulatorMs));
}

function sendConstruction(): void {
  const choices = simulation.construction_choices();
  const rows = choices ? choices.split("\n").map((row) => row.split("\t")) : [];
  const pieces: ConstructionPiece[] = rows.filter((fields) => !fields[6]).map((fields) => {
    const [id, label, group, heading, slope, bank] = fields;
    return { id, label, group, heading, slope, bank };
  });
  const selected = simulation.construction_selected();
  const selectedChoice = rows.find((fields) => fields[0] === selected);
  const track = simulation.construction_track();
  const previewRails = simulation.construction_rails();
  const previewTies = simulation.construction_ties();
  const previewSupports = simulation.construction_supports();
  const placedRails = simulation.construction_placed_rails();
  const placedTies = simulation.construction_placed_ties();
  const placedSupports = simulation.construction_placed_supports();
  lastConstructionMessage = simulation.construction_message();
  workerScope.postMessage({ type: "construction-state", active: simulation.construction_active(), pieces,
    selected: selected || null, track: track ? track.split("\n").map((row) => {
      const [id, label] = row.split("\t");
      return { id: Number(id), label };
    }) : [],
    canAppend: !!selectedChoice && !selectedChoice[6], canClose: simulation.construction_can_close(),
    canTest: simulation.construction_can_test(), message: lastConstructionMessage || selectedChoice?.[6] || "",
    previewRails, previewTies, previewSupports, placedRails, placedTies, placedSupports,
  }, [previewRails.buffer, previewTies.buffer, previewSupports.buffer, placedRails.buffer, placedTies.buffer, placedSupports.buffer]);
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
    const action = event.data;
    if (action.type !== "set-surface") {
      switch (action.type) {
        case "construction-start": simulation.construction_start(action.x * 2, action.y * 2, action.z / 2, action.heading * 2); break;
        case "construction-select": simulation.construction_select(action.piece); break;
        case "construction-append": simulation.construction_append(); break;
        case "construction-undo": simulation.construction_undo(); break;
        case "construction-delete": simulation.construction_delete(action.id); break;
        case "construction-close": simulation.construction_close(); break;
        case "construction-test": simulation.construction_test(); break;
        case "construction-delete-mode": return;
      }
      sendConstruction();
      return;
    }
    validateSurfaceEdit(action, simulation.world_dimensions());
    const { x, y, corners, water } = action;
    const changed = simulation.set_world_surface(x, y, corners, water);
    if (changed) {
      sendWorld();
      if (simulation.construction_active()) sendConstruction();
    }
    workerScope.postMessage({ type: "surface-edited", changed });
  });
});
