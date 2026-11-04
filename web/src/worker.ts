import type { ConstructionColorsWire as ConstructionColors } from "./generated/ConstructionColorsWire";
import type { Facility, FacilityAction } from "./facilities";
import init, { WasmSimulation, piece_shapes_json } from "./wasm/theme_park_sim_wasm.js";
import { captureWorldSnapshot, validateSurfaceEdit, type SetSurfaceMessage } from "./world-snapshot";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";
import type { ConstructionAction, ConstructionElement, ConstructionPiece, ConstructionTrack, PieceShape } from "./construction-protocol";
import type { Scenery, SceneryCommand } from "./scenery-tool";
import type { TerrainEditCommand } from "./landscaping-tool";
import { validateBlockAnalysisBudget } from "./game-test";
import { pathPreviewMessage } from "./render/path-ghost";
import { SessionRecorder } from "./session-recording";
import { parseCoasterColors } from "./coaster-colors";

import { sendWorkerMessage, assertNever, type WorkerAction, type WorkerMessage, type PathAction, type PathPosition } from "./protocol";
import type { BlockReport } from "./construction-protocol";
import type { TestingReport } from "./coaster-testing";
import type { GuestDetail } from "./guest-panel/protocol";

const tickRate = 30;
let manualBlockAnalysis = false;
let simulationPaused = false;
let gameSpeed = 1;
const workerScope = self as unknown as {
  postMessage(message: WorkerMessage, transfer?: Transferable[]): void;
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerAction>) => void): void;
};
let simulation: WasmSimulation;
let recorder: SessionRecorder;
let clock = createSimulationClock(tickRate, 4);
let lastWakeup: number;
let coasterGeometrySent = false;
let lastConstructionMessage = "";
let lastBlockReport = "";
let lastConstructionEconomy = "";
let selectedGuestId: number | undefined;
let lastGuestDetailSentAt = 0;
let lastFacilityRevenue = "";
let lastFacilitiesJson = "";
let blockAnalysisScheduled = false;

function tickSimulation(): void {
  simulation.tick();
  const tick = Number(simulation.tick_count());
  if (tick % 300 === 0) recorder.recordCheckpoint(tick, simulation.state_hash());
}

function scheduleBlockAnalysis(): void {
  if (manualBlockAnalysis || blockAnalysisScheduled) return;
  blockAnalysisScheduled = true;
  setTimeout(() => {
    blockAnalysisScheduled = false;
    const complete = advanceBlockAnalysis(1000);
    if (!complete) scheduleBlockAnalysis();
  }, 1000 / tickRate);
}

function advanceBlockAnalysis(budget: number): boolean {
  recorder.recordCommand(Number(simulation.tick_count()), { type: "test-advance-block-analysis", budget });
  const complete = simulation.construction_advance_block_analysis(budget);
  const report = simulation.construction_block_report_json();
  if (simulation.construction_active() && report !== lastBlockReport) {
    lastBlockReport = report;
    lastConstructionMessage = simulation.construction_message();
    sendWorkerMessage(workerScope, { type: "block-analysis-progress", blockReport: JSON.parse(report) as BlockReport, trainCount: simulation.construction_train_count(), message: lastConstructionMessage });
  }
  return complete;
}

function wake(forceSnapshot = false): void {
  const now = performance.now();
  const result = advanceSimulation(clock, now - lastWakeup, gameSpeed);
  lastWakeup = now;
  clock = result.clock;
  if (!simulationPaused) {
    for (let tick = 0; tick < result.ticksToRun; tick += 1) tickSimulation();
  }

  if (forceSnapshot || simulationPaused || result.ticksToRun > 0) {
    const guestIds = simulation.guest_ids();
    const guests = simulation.snapshot();
    const appearances = simulation.guest_appearances();
    const trainPoses = simulation.coaster_train_poses();
    const constructionTrainPoses = simulation.construction_active() ? simulation.construction_train_poses() : new Float32Array();
    const constructionRiders = simulation.construction_riders();
    sendWorkerMessage(workerScope,
      { type: "snapshot", sentAt: performance.now(), tickCount: Number(simulation.tick_count()), guestIds, guests, appearances, trainPoses, constructionTrainPoses, constructionRiders },
      [guestIds.buffer, guests.buffer, appearances.buffer, trainPoses.buffer, constructionTrainPoses.buffer, constructionRiders.buffer],
    );
  }
  const message = simulation.construction_message();
  const constructionEconomy = `${simulation.construction_coaster_price()}:${simulation.construction_coaster_income()}`;
  if (simulation.construction_active() && (message !== lastConstructionMessage || simulation.construction_block_report_json() !== lastBlockReport || constructionEconomy !== lastConstructionEconomy)) sendConstruction();
  if (selectedGuestId !== undefined && now - lastGuestDetailSentAt >= 500) sendGuestDetail();
  if (simulation.facilities_json() !== lastFacilitiesJson || String(simulation.facility_revenue()) !== lastFacilityRevenue) sendFacilities();
  if (simulationPaused) return;
  setTimeout(wake, Math.max(0, clock.tickDurationMs - clock.accumulatorMs));
}

function sendGuestDetail(guestId = selectedGuestId): void {
  if (guestId === undefined) return;
  const detail = JSON.parse(simulation.guest_detail_json(guestId)) as GuestDetail | null;
  sendWorkerMessage(workerScope, { type: "guest-detail", guestId: `guest-${guestId}`, detail });
  lastGuestDetailSentAt = performance.now();
}

function sendConstruction(): void {
  const pieces = JSON.parse(simulation.construction_choices()) as ConstructionPiece[];
  const elements = JSON.parse(simulation.construction_elements()) as ConstructionElement[];
  const selected = simulation.construction_selected();
  const selectedChoice = pieces.find((piece) => piece.id === selected);
  const track = JSON.parse(simulation.construction_track()) as ConstructionTrack[];
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
  const coasterPrice = simulation.construction_coaster_price();
  const coasterIncome = simulation.construction_coaster_income();
  lastConstructionEconomy = `${coasterPrice}:${coasterIncome}`;
  const colorNames = JSON.parse(simulation.construction_colors()) as ConstructionColors | null;
  sendWorkerMessage(workerScope, { type: "construction-state", active: simulation.construction_active(), chain: simulation.construction_chain(), colors: colorNames ? parseCoasterColors(colorNames) : null, pieces, elements,
    selected: selected || null, track,
    canAppend: simulation.construction_finish_preview_active() || elements.some((element) => element.id === selected) || (!!selectedChoice && !selectedChoice.refusal), canClose: simulation.construction_can_close(),
    finishPreview: simulation.construction_finish_preview_active(),
    canFinish: simulation.construction_can_finish(), finishReason: simulation.construction_finish_reason(),
    testingReport: JSON.parse(simulation.construction_test_report_json()) as TestingReport | null,
    canTest: simulation.construction_can_test(), isOpen: simulation.construction_is_open(), coasterPrice, coasterIncome, stationPositions: [...simulation.construction_station_positions()], message: lastConstructionMessage || selectedChoice?.refusal || "",
    blockReport: JSON.parse(lastBlockReport) as BlockReport, trainCount: simulation.construction_train_count(),
    previewRails, previewTies, previewSupports, placedRails, placedTies, placedSupports, railDistances, blockCandidates,
  }, [previewRails.buffer, previewTies.buffer, previewSupports.buffer, placedRails.buffer, placedTies.buffer, placedSupports.buffer, railDistances.buffer, blockCandidates.buffer]);
}

let finishSearchGeneration = 0;
let finishSearchPending = false;
let finishSearchAwaited = false;
function scheduleFinishSearch(): void {
  const generation = ++finishSearchGeneration;
  finishSearchPending = true;
  setTimeout(() => {
    if (generation !== finishSearchGeneration) return;
    simulation.construction_finish_search_start();
    sendConstruction();
    const advance = (): void => {
      if (generation !== finishSearchGeneration) return;
      if (simulation.construction_finish_search_advance()) {
        finishSearchPending = false;
        sendConstruction();
        if (finishSearchAwaited) {
          finishSearchAwaited = false;
          sendWorkerMessage(workerScope, { type: "construction-finish-ready" });
        }
      }
      else setTimeout(advance, 0);
    };
    advance();
  }, 0);
}

function sendFacilities(message = "", placed = false, forced = false): void {
  lastFacilitiesJson = simulation.facilities_json();
  const facilities = JSON.parse(lastFacilitiesJson) as Facility[];
  lastFacilityRevenue = String(simulation.facility_revenue());
  sendWorkerMessage(workerScope, { type: "facilities", facilities, message, placed, forced, revenue: lastFacilityRevenue });
}

function sendScenery(message = ""): void {
  const scenery = JSON.parse(simulation.scenery_json()) as Scenery[];
  sendWorkerMessage(workerScope, { type: "scenery", scenery, message });
}

function sendWorld(): void {
  const world = captureWorldSnapshot(simulation);
  const coasterSupports = simulation.coaster_supports();
  if (!coasterGeometrySent) {
    const coasterGeometry = simulation.coaster_geometry();
    const coasterRails = simulation.coaster_rails();
    const coasterTies = simulation.coaster_ties();
    const demoColors = JSON.parse(simulation.coaster_colors()) as ConstructionColors | null;
    const coasterColors = demoColors ? parseCoasterColors(demoColors) : null;
    sendWorkerMessage(workerScope, { ...world.message, coasterGeometry, coasterRails, coasterTies, coasterSupports, coasterColors }, [...world.transfer, coasterGeometry.buffer, coasterRails.buffer, coasterTies.buffer, coasterSupports.buffer]);
    coasterGeometrySent = true;
  } else {
    sendWorkerMessage(workerScope, { ...world.message, coasterSupports }, [...world.transfer, coasterSupports.buffer]);
  }
}

function validatePathPosition(position: PathPosition): void {
  if (!Number.isInteger(position.x) || !Number.isInteger(position.y) || !Number.isInteger(position.z) ||
      position.x < 0 || position.y < 0 || position.x > 65535 || position.y > 65535 ||
      position.z < -32768 || position.z > 32767) {
    throw new RangeError("Invalid path position");
  }
}

function validatePathPositionInWorld(position: PathPosition, dimensions: Uint16Array): void {
  validatePathPosition(position);
  if (position.x >= dimensions[0] || position.y >= dimensions[1]) throw new RangeError("Invalid path position");
}

function validatePathRoute(action: Extract<PathAction, { type: "path-preview" | "path-build" }>): void {
  validatePathPosition(action.start);
  validatePathPosition(action.end);
  if (!Number.isInteger(action.style) || action.style < 0 || action.style > 255) {
    throw new RangeError("Invalid path style");
  }
}

function pathRouteCoordinates(start: PathPosition, end: PathPosition): Int32Array {
  return Int32Array.of(start.x, start.y, start.z, end.x, end.y, end.z);
}

let initialized: Promise<void> | undefined;
workerScope.addEventListener("message", (event) => {
  const action = event.data;
  if (action.type === "init") {
    if (initialized) throw new Error("Worker already initialized");
    manualBlockAnalysis = action.manualBlockAnalysis;
    simulationPaused = action.frozen;
    initialized = init().then(() => {
      simulation = new WasmSimulation(action.seed, action.guestCount, action.park);
      recorder = new SessionRecorder({ seed: action.seed.toString(), park: action.park, guestCount: action.guestCount });
      sendWorld();
      sendFacilities();
      sendScenery();
      sendWorkerMessage(workerScope, { type: "piece-shapes", shapes: JSON.parse(piece_shapes_json()) as PieceShape[] });
      sendWorkerMessage(workerScope, { type: "ready" });
      lastWakeup = performance.now();
      wake(true);
    });
    return;
  }
  if (!initialized) throw new Error("Worker must be initialized first");
  void initialized.then(() => {
    if (action.type === "export-session") {
      sendWorkerMessage(workerScope, { type: "session-export", requestId: action.requestId, recording: recorder.snapshot(Number(simulation.tick_count())) });
      return;
    }
    if (action.type === "construction-finish-await") {
      if (finishSearchPending) finishSearchAwaited = true;
      else sendWorkerMessage(workerScope, { type: "construction-finish-ready" });
      return;
    }
    if (!["test-resume", "test-advance", "test-advance-block-analysis", "path-preview", "facility-access-preview", "guest-detail-request", "guest-detail-close", "construction-delete-mode", "construction-finish-preview", "construction-finish-confirm", "construction-finish-cancel"].includes(action.type)) {
      simulation.construction_finish_invalidate();
      scheduleFinishSearch();
    }
    if (!["test-resume", "test-advance", "test-advance-block-analysis", "path-preview", "facility-access-preview", "guest-detail-request", "guest-detail-close", "construction-delete-mode"].includes(action.type)) {
      recorder.recordCommand(Number(simulation.tick_count()), action);
    }
    if (action.type === "test-resume") {
      if (!simulationPaused || !manualBlockAnalysis) throw new Error("Test resume requires a frozen worker with manual block analysis");
      simulationPaused = false;
      manualBlockAnalysis = false;
      clock = createSimulationClock(tickRate, 4);
      lastWakeup = performance.now();
      sendWorkerMessage(workerScope, { type: "test-resumed" });
      wake(true);
      scheduleBlockAnalysis();
      return;
    }
    if (action.type === "set-speed") {
      if (!Number.isInteger(action.speed) || action.speed < 1 || action.speed > 16) throw new RangeError("Invalid game speed");
      gameSpeed = action.speed;
      return;
    }
    if (action.type === "test-advance-block-analysis") {
      if (!manualBlockAnalysis) throw new Error("Test block analysis advancement is disabled");
      validateBlockAnalysisBudget(action.budget);
      advanceBlockAnalysis(action.budget);
      sendWorkerMessage(workerScope, { type: "test-block-analysis-advanced" });
      return;
    }
    if (action.type === "test-advance") {
      if (!Number.isInteger(action.count) || action.count < 1 || action.count > 100000) throw new RangeError("Invalid test tick count");
      for (let tick = 0; tick < action.count; tick++) tickSimulation();
      wake();
      sendFacilities("", false, true);
      sendWorkerMessage(workerScope, { type: "test-advanced" });
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
    if (action.type === "facility-access-preview") {
      const rotation = simulation.facility_access_rotation(action.kind, action.x, action.y, action.height, action.rotation);
      sendWorkerMessage(workerScope, { type: "facility-access-preview", requestId: action.requestId, rotation: rotation < 0 ? undefined : rotation });
      return;
    }
    if (action.type === "facility-place") {
      let placed: boolean;
      if (action.height === undefined) placed = simulation.place_facility(action.kind, action.x, action.y, action.rotation);
      else {
        const inferredRotation = simulation.facility_access_rotation(action.kind, action.x, action.y, action.height, action.rotation);
        const rotation = inferredRotation < 0 ? action.rotation : inferredRotation;
        placed = simulation.place_facility_at(action.kind, action.x, action.y, rotation, action.height);
      }
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
      sendWorkerMessage(workerScope, { type: "terrain-result", success, message: simulation.terrain_message(), invalidatedPaths }, [invalidatedPaths.buffer]);
      return;
    }
    if (action.type === "guest-detail-request") {
      const match = /^guest-(\d+)$/.exec(action.guestId);
      if (!match) {
        sendWorkerMessage(workerScope, { type: "guest-detail", guestId: action.guestId, detail: null });
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
      validatePathRoute(action);
      const paths = simulation.path_preview(
        pathRouteCoordinates(action.start, action.end),
        action.kind,
        action.style,
      );
      sendWorkerMessage(workerScope,
        { type: "path-preview", requestId: action.requestId, valid: paths.length > 0 && paths.every((value, index) => index % 9 !== 6 || value === 0), paths, message: pathPreviewMessage(paths) },
        [paths.buffer],
      );
      return;
    }
    if (action.type === "path-build") {
      validatePathRoute(action);
      const success = simulation.path_build(
        pathRouteCoordinates(action.start, action.end),
        action.kind,
        action.style,
      );
      if (success) sendWorld();
      sendWorkerMessage(workerScope, {
        type: "path-command-result",
        requestId: action.requestId,
        command: "build",
        success,
        message: simulation.path_message() || (success ? "Path built." : "Path build failed."),
      });
      return;
    }
    if (action.type === "path-remove") {
      validatePathPositionInWorld({ x: action.x, y: action.y, z: action.z }, simulation.world_dimensions());
      const success = simulation.path_remove(action.x, action.y, action.z);
      if (success) sendWorld();
      sendWorkerMessage(workerScope, {
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
        case "construction-set-chain": simulation.construction_set_chain(action.enabled); break;
        case "construction-append": simulation.construction_append(); break;
        case "construction-undo": simulation.construction_undo(); break;
        case "construction-delete": simulation.construction_delete(action.id); break;
        case "construction-close": simulation.construction_close(); break;
        case "construction-finish-preview": simulation.construction_finish_preview(); break;
        case "construction-finish-confirm": simulation.construction_finish_confirm(); scheduleFinishSearch(); break;
        case "construction-finish-cancel": simulation.construction_finish_cancel(); break;
        case "construction-set-open": simulation.construction_set_open(action.open); break;
        case "construction-set-coaster-price": simulation.construction_set_coaster_price(action.price); break;
        case "construction-set-colors": simulation.construction_set_colors(action.colors.track, action.colors.supports, action.colors.trains); break;
        case "construction-test": simulation.construction_test(); break;
        case "construction-auto-blocks": simulation.construction_auto_blocks(); break;
        case "construction-apply-best-blocks": simulation.construction_apply_best_block_sections(); break;
        case "construction-set-train-count": simulation.construction_set_train_count(action.count); break;
        case "construction-move-block": simulation.construction_move_block_section(action.index, action.boundaryMetres); break;
        case "construction-remove-block": simulation.construction_remove_block_section(action.index); break;
        case "construction-add-block": simulation.construction_add_block_section(action.boundaryMetres); break;
        case "construction-delete-mode": return;
        default: assertNever(action);
      }
      sendConstruction();
      scheduleBlockAnalysis();
      if (["construction-start", "construction-append", "construction-undo", "construction-delete", "construction-close", "construction-finish-confirm", "construction-set-open"].includes(action.type)) sendWorld();
      return;
    }
    validateSurfaceEdit(action, simulation.world_dimensions());
    const { x, y, corners, water } = action;
    const changed = simulation.set_world_surface(x, y, corners, water);
    if (changed) {
      sendWorld();
      if (simulation.construction_active()) sendConstruction();
    }
    sendWorkerMessage(workerScope, { type: "surface-edited", changed });
  });
});
