import type { Facility, FacilityAction } from "./facilities";
import init, { WasmSimulation, piece_shapes_json } from "./wasm/theme_park_sim_wasm.js";
import { captureWorldSnapshot, validateSurfaceEdit, type SetSurfaceMessage } from "./world-snapshot";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";
import type { ConstructionAction, ConstructionPiece, PieceShape } from "./construction-protocol";
import type { Scenery, SceneryCommand } from "./scenery-tool";
import type { TerrainEditCommand } from "./landscaping-tool";

type GuestDetailAction =
  | { readonly type: "guest-detail-request"; readonly guestId: string }
  | { readonly type: "guest-detail-close" };
interface PathPosition {
  readonly x: number;
  readonly y: number;
  /** Height in quarter-tile steps. */
  readonly z: number;
}
type PathAction =
  | { readonly type: "path-preview"; readonly requestId?: number; readonly start: PathPosition; readonly end: PathPosition; readonly kind: "normal" | "queue"; readonly style: number }
  | { readonly type: "path-build"; readonly requestId: number; readonly start: PathPosition; readonly end: PathPosition; readonly kind: "normal" | "queue"; readonly style: number }
  | { readonly type: "path-remove"; readonly requestId: number; readonly x: number; readonly y: number; readonly z: number };
type WorkerAction = SceneryCommand | SetSurfaceMessage | TerrainEditCommand | PathAction | ConstructionAction | GuestDetailAction | FacilityAction | { type: "facility-remove"; id: number } | { type: "test-advance"; count: number } | { type: "facility-price"; id: number; price: number };

const tickRate = 30;
const workerName = (self as unknown as { name?: string }).name ?? "";
const testMode = workerName === "theme-park-test" || workerName.startsWith("theme-park-test:");
const testGuestCount = workerName.startsWith("theme-park-test:") ? Number(workerName.slice("theme-park-test:".length)) : 12;
if (!Number.isInteger(testGuestCount) || testGuestCount < 1 || testGuestCount > 500) throw new RangeError("Invalid test guest count");
const workerScope = self as unknown as {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerAction>) => void): void;
};
let simulation: WasmSimulation;
let clock = createSimulationClock(tickRate, 4);
let simulationPaused = testMode;
let lastWakeup: number;
let coasterGeometrySent = false;
let lastConstructionMessage = "";
let lastBlockReport = "";
let selectedGuestId: number | undefined;
let lastGuestDetailSentAt = 0;
let lastFacilityRevenue = "";
let lastFacilitiesJson = "";

function wake(): void {
  const now = performance.now();
  const result = advanceSimulation(clock, now - lastWakeup);
  lastWakeup = now;
  clock = result.clock;
  if (!simulationPaused) {
    for (let tick = 0; tick < result.ticksToRun; tick += 1) simulation.tick();
  }

  const guestIds = simulation.guest_ids();
  const guests = simulation.snapshot();
  const appearances = simulation.guest_appearances();
  const trainPoses = simulation.coaster_train_poses();
  const constructionTrainPoses = simulation.construction_active() ? simulation.construction_train_poses() : new Float32Array();
  const constructionRiders = simulation.construction_riders();
  workerScope.postMessage(
    { type: "snapshot", tickCount: Number(simulation.tick_count()), guestIds, guests, appearances, trainPoses, constructionTrainPoses, constructionRiders },
    [guestIds.buffer, guests.buffer, appearances.buffer, trainPoses.buffer, constructionTrainPoses.buffer, constructionRiders.buffer],
  );
  const message = simulation.construction_message();
  if (simulation.construction_active() && (message !== lastConstructionMessage || simulation.construction_block_report_json() !== lastBlockReport)) sendConstruction();
  if (selectedGuestId !== undefined && now - lastGuestDetailSentAt >= 500) sendGuestDetail();
  if (simulation.facilities_json() !== lastFacilitiesJson || String(simulation.facility_revenue()) !== lastFacilityRevenue) sendFacilities();
  if (simulationPaused) return;
  setTimeout(wake, Math.max(0, clock.tickDurationMs - clock.accumulatorMs));
}

function sendGuestDetail(guestId = selectedGuestId): void {
  if (guestId === undefined) return;
  const detail = JSON.parse(simulation.guest_detail_json(guestId)) as unknown;
  workerScope.postMessage({ type: "guest-detail", guestId: `guest-${guestId}`, detail });
  lastGuestDetailSentAt = performance.now();
}

function sendConstruction(): void {
  const choices = simulation.construction_choices();
  const rows = choices ? choices.split("\n").map((row) => row.split("\t")) : [];
  const pieces: ConstructionPiece[] = rows.map(([id, label, group, heading, slope, bank, refusal]) => ({ id, label, group, heading, slope, bank, refusal }));
  const selected = simulation.construction_selected();
  const selectedChoice = rows.find((fields) => fields[0] === selected);
  const track = simulation.construction_track();
  const previewRails = simulation.construction_rails();
  const previewTies = simulation.construction_ties();
  const previewSupports = simulation.construction_supports();
  const placedRails = simulation.construction_placed_rails();
  const railDistances = simulation.construction_rail_distances();
  const blockCandidates = simulation.construction_block_candidates();
  const placedTies = simulation.construction_placed_ties();
  const placedSupports = simulation.construction_placed_supports();
  lastConstructionMessage = simulation.construction_message();
  lastBlockReport = simulation.construction_block_report_json();
  workerScope.postMessage({ type: "construction-state", active: simulation.construction_active(), pieces,
    selected: selected || null, track: track ? track.split("\n").map((row) => {
      const [id, label] = row.split("\t");
      return { id: Number(id), label };
    }) : [],
    canAppend: !!selectedChoice && !selectedChoice[6], canClose: simulation.construction_can_close(),
    testingReport: JSON.parse(simulation.construction_test_report_json()) as unknown,
    canTest: simulation.construction_can_test(), isOpen: simulation.construction_is_open(), stationPositions: [...simulation.construction_station_positions()], message: lastConstructionMessage || selectedChoice?.[6] || "",
    blockReport: JSON.parse(lastBlockReport) as unknown, trainCount: simulation.construction_train_count(),
    previewRails, previewTies, previewSupports, placedRails, placedTies, placedSupports, railDistances, blockCandidates,
  }, [previewRails.buffer, previewTies.buffer, previewSupports.buffer, placedRails.buffer, placedTies.buffer, placedSupports.buffer, railDistances.buffer, blockCandidates.buffer]);
}

function sendFacilities(message = "", placed = false, forced = false): void {
  lastFacilitiesJson = simulation.facilities_json();
  const facilities = JSON.parse(lastFacilitiesJson) as Facility[];
  lastFacilityRevenue = String(simulation.facility_revenue());
  workerScope.postMessage({ type: "facilities", facilities, message, placed, forced, revenue: lastFacilityRevenue });
}

function sendScenery(message = ""): void {
  const scenery = JSON.parse(simulation.scenery_json()) as Scenery[];
  workerScope.postMessage({ type: "scenery", scenery, message });
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

function validatePathPosition(position: PathPosition, dimensions: Uint16Array): void {
  if (!Number.isInteger(position.x) || !Number.isInteger(position.y) || !Number.isInteger(position.z) ||
      position.x < 0 || position.y < 0 || position.x >= dimensions[0] || position.y >= dimensions[1] ||
      position.z < -32768 || position.z > 32767) {
    throw new RangeError("Invalid path position");
  }
}

function validatePathRoute(action: Extract<PathAction, { type: "path-preview" | "path-build" }>, dimensions: Uint16Array): void {
  validatePathPosition(action.start, dimensions);
  validatePathPosition(action.end, dimensions);
  if (!Number.isInteger(action.style) || action.style < 0 || action.style > 255) {
    throw new RangeError("Invalid path style");
  }
}

function pathRouteCoordinates(start: PathPosition, end: PathPosition): Int32Array {
  return Int32Array.of(start.x, start.y, start.z, end.x, end.y, end.z);
}

const initialized = init().then(() => {
  simulation = new WasmSimulation(2026n, testGuestCount);
  sendWorld();
  sendFacilities();
  sendScenery();
  workerScope.postMessage({ type: "piece-shapes", shapes: JSON.parse(piece_shapes_json()) as PieceShape[] });
  workerScope.postMessage({ type: "ready" });
  lastWakeup = performance.now();
  wake();
});

workerScope.addEventListener("message", (event) => {
  void initialized.then(() => {
    const action = event.data;
    if (action.type === "test-advance") {
      if (!Number.isInteger(action.count) || action.count < 1 || action.count > 100000) throw new RangeError("Invalid test tick count");
      for (let tick = 0; tick < action.count; tick++) simulation.tick();
      wake();
      sendFacilities("", false, true);
      workerScope.postMessage({ type: "test-advanced" });
      return;
    }
    if (action.type === "scenery-place" || action.type === "scenery-remove") {
      const success = action.type === "scenery-place"
        ? simulation.place_scenery(action.kind, action.x, action.y, action.rotation, action.quarter)
        : simulation.remove_scenery(action.id);
      if (success) sendWorld();
      sendScenery(simulation.scenery_message() || (success ? (action.type === "scenery-place" ? "Scenery placed." : "Scenery removed.") : "Scenery could not be changed."));
      return;
    }
    if (action.type === "facility-price") {
      const changed = simulation.set_facility_price(action.id, action.price);
      sendFacilities(changed ? "Price updated." : simulation.facility_message());
      return;
    }
    if (action.type === "facility-remove") {
      const removed = simulation.remove_facility(action.id);
      if (removed) sendWorld();
      sendFacilities(removed ? "Facility removed." : simulation.facility_message());
      return;
    }
    if (action.type === "facility-place") {
      const placed = action.height === undefined
        ? simulation.place_facility(action.kind, action.x, action.y, action.rotation)
        : simulation.place_facility_at(action.kind, action.x, action.y, action.rotation, action.height);
      if (placed) sendWorld();
      sendFacilities(placed ? "Facility placed." : simulation.facility_message(), placed);
      return;
    }
    if (action.type === "terrain-edit") {
      const values = [action.x, action.y, action.size, action.corner, action.steps, action.height, action.bottom, action.top];
      if (!values.every(Number.isInteger) || action.x < 0 || action.y < 0 || action.x > 65535 || action.y > 65535 ||
          action.size < 1 || action.size > 9 || action.corner < 0 || action.corner > 3 ||
          values.slice(4).some((value) => value < -32768 || value > 32767)) throw new RangeError("Invalid terrain edit");
      const success = simulation.terrain_apply(action.command, Int32Array.from(values));
      if (success) {
        sendWorld();
        if (simulation.construction_active()) sendConstruction();
      }
      const invalidatedPaths = simulation.terrain_invalidated_paths();
      workerScope.postMessage({ type: "terrain-result", success, message: simulation.terrain_message(), invalidatedPaths }, [invalidatedPaths.buffer]);
      return;
    }
    if (action.type === "guest-detail-request") {
      const match = /^guest-(\d+)$/.exec(action.guestId);
      if (!match) {
        workerScope.postMessage({ type: "guest-detail", guestId: action.guestId, detail: null });
        return;
      }
      selectedGuestId = Number(match[1]);
      sendGuestDetail();
      return;
    }
    if (action.type === "guest-detail-close") {
      selectedGuestId = undefined;
      return;
    }
    if (action.type === "path-preview") {
      validatePathRoute(action, simulation.world_dimensions());
      const paths = simulation.path_preview(
        pathRouteCoordinates(action.start, action.end),
        action.kind,
        action.style,
      );
      workerScope.postMessage(
        { type: "path-preview", requestId: action.requestId, valid: paths.length > 0 && paths.every((value, index) => index % 9 !== 6 || value === 0), paths, message: simulation.path_message() },
        [paths.buffer],
      );
      return;
    }
    if (action.type === "path-build") {
      validatePathRoute(action, simulation.world_dimensions());
      const success = simulation.path_build(
        pathRouteCoordinates(action.start, action.end),
        action.kind,
        action.style,
      );
      if (success) sendWorld();
      workerScope.postMessage({
        type: "path-command-result",
        requestId: action.requestId,
        command: "build",
        success,
        message: simulation.path_message() || (success ? "Path built." : "Path build failed."),
      });
      return;
    }
    if (action.type === "path-remove") {
      validatePathPosition({ x: action.x, y: action.y, z: action.z }, simulation.world_dimensions());
      const success = simulation.path_remove(action.x, action.y, action.z);
      if (success) sendWorld();
      workerScope.postMessage({
        type: "path-command-result",
        requestId: action.requestId,
        command: "remove",
        success,
        message: simulation.path_message() || (success ? "Path removed." : "Path removal failed."),
      });
      return;
    }
    if (action.type !== "set-surface") {
      switch (action.type) {
        case "construction-start": simulation.construction_start(action.x * 2, action.y * 2, action.z / 2, action.heading * 2); break;
        case "construction-select": simulation.construction_select(action.piece); break;
        case "construction-append": simulation.construction_append(); break;
        case "construction-undo": simulation.construction_undo(); break;
        case "construction-delete": simulation.construction_delete(action.id); break;
        case "construction-close": simulation.construction_close(); break;
        case "construction-set-open": simulation.construction_set_open(action.open); break;
        case "construction-test": simulation.construction_test(); break;
        case "construction-auto-blocks": simulation.construction_auto_blocks(); break;
        case "construction-apply-best-blocks": simulation.construction_apply_best_block_sections(); break;
        case "construction-set-train-count": simulation.construction_set_train_count(action.count); break;
        case "construction-move-block": simulation.construction_move_block_section(action.index, action.boundaryMetres); break;
        case "construction-remove-block": simulation.construction_remove_block_section(action.index); break;
        case "construction-add-block": simulation.construction_add_block_section(action.boundaryMetres); break;
        case "construction-delete-mode": return;
      }
      sendConstruction();
      if (["construction-start", "construction-append", "construction-undo", "construction-delete", "construction-close", "construction-set-open"].includes(action.type)) sendWorld();
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
