import type { ConstructionColorsWire as ConstructionColors } from "./generated/ConstructionColorsWire";
import type { Facility, FacilityAction } from "./facilities";
import init, { BuilderSessionBridge, WasmSimulation, piece_shapes_json } from "./wasm/theme_park_sim_wasm.js";
import { captureWorldSnapshot, validateSurfaceEdit, type SetSurfaceMessage } from "./world-snapshot";
import { advanceSimulation, createSimulationClock } from "./simulation-loop";
import type { ConstructionTrack, PieceShape } from "./construction-protocol";
import type { Scenery, SceneryCommand } from "./scenery-tool";
import type { TerrainEditCommand } from "./landscaping-tool";
import { validateBlockAnalysisBudget } from "./game-test";
import { pathPreviewMessage } from "./render/path-ghost";
import { SessionRecorder } from "./session-recording";
import { coasterListEntries } from "./coaster-list-data";
import { parseCoasterColors } from "./coaster-colors";
import { parseTrainLayout, type TrainLayout } from "./train-layout";
import { shouldInvalidateConstructionFinishSearch } from "./construction-finish-search";
import { applyConstructionAction, readBuilderEstimates, sendsIncompleteConstructionState } from "./worker-construction";
import type { ConstructionFinishOptionsWire } from "./generated/ConstructionFinishOptionsWire";
import type { RideInfoReport } from "./generated/RideInfoReport";

import { sendWorkerMessage, type WorkerAction, type WorkerMessage, type PathAction, type PathPosition, type PlacementOutcome, type CoasterListEntry, type CoasterRenderManifest, DEMO_COASTER_ID } from "./protocol";
import type { BlockReport } from "./construction-protocol";
import type { TestingReport } from "./coaster-testing";
import type { GuestDetail } from "./guest-panel/protocol";
import type { RideInfo, RideKey } from "./ride-window/protocol";

const tickRate = 30;
const finishSearchSliceMs = 4;
let manualBlockAnalysis = false;
let testMode = false;
let simulationPaused = false;
let gamePaused = false;
let gameSpeed = 1;
const workerScope = self as unknown as {
  postMessage(message: WorkerMessage, transfer?: Transferable[]): void;
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerAction>) => void): void;
};
let outputScope: { postMessage(message: WorkerMessage, transfer?: Transferable[]): void } = workerScope;
let simulation: WasmSimulation;
let builderSession: BuilderSessionBridge;
let recorder: SessionRecorder | undefined;
let sessionRecordingAvailable = true;
let clock = createSimulationClock(tickRate, 4);
let lastWakeup: number;
let coasterTrackRevisions = new Map<number, string>();
let lastConstructionMessage = "";
let lastBlockReport = "";
let lastConstructionEconomy = "";
let lastCoastersJson = "";
let lastCoasters: CoasterListEntry[] = [];
let selectedGuestId: number | undefined;
let selectedRide: RideKey | undefined;
let lastGuestDetailSentAt = 0;
let lastFacilityRevenue = "";
let lastFacilitiesJson = "";
let blockAnalysisScheduled = false;
let initializationError: string | undefined;

function recordSessionCommand<Command extends { type: string }>(command: Command): void {
  if (!sessionRecordingAvailable) return;
  if (!recorder) throw new Error("Session recorder is unavailable for this park");
  recorder.recordCommand(Number(simulation.tick_count()), command);
}

function sessionRecorder(): SessionRecorder {
  if (!sessionRecordingAvailable || !recorder) throw new Error("Session recording is unavailable for this park");
  return recorder;
}

function publishInitializationError(error: unknown): void {
  initializationError = error instanceof Error ? error.message : String(error);
  sendWorkerMessage(outputScope, { type: "initialization-error", error: initializationError });
}

function tickSimulation(): void {
  simulation.tick();
  const tick = Number(simulation.tick_count());
  if (sessionRecordingAvailable && tick % 300 === 0) sessionRecorder().recordCheckpoint(tick, simulation.state_hash());
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
  recordSessionCommand({ type: "test-advance-block-analysis", budget });
  const complete = simulation.construction_advance_block_analysis(budget);
  sendBlockAnalysisProgress();
  return complete;
}

function sendBlockAnalysisProgress(): void {
  const report = simulation.construction_block_report_json();
  if (simulation.construction_active() && report !== lastBlockReport) {
    lastBlockReport = report;
    lastConstructionMessage = simulation.construction_message();
    sendWorkerMessage(outputScope, { type: "block-analysis-progress", blockReport: JSON.parse(report) as BlockReport, trainCount: simulation.construction_train_count(), message: lastConstructionMessage });
  }
}

function wake(forceSnapshot = false, scheduleNext = true): void {
  const now = performance.now();
  const result = advanceSimulation(clock, now - lastWakeup, gameSpeed);
  lastWakeup = now;
  clock = result.clock;
  if (!simulationPaused && !gamePaused) {
    for (let tick = 0; tick < result.ticksToRun; tick += 1) tickSimulation();
  }

  if (forceSnapshot || simulationPaused || result.ticksToRun > 0) {
    const guestIds = simulation.guest_ids();
    const guests = simulation.snapshot();
    const appearances = simulation.guest_appearances();
    const coasters = Array.from(simulation.coaster_ids(), (id) => ({
      id,
      trainPoses: simulation.coaster_train_poses(id),
      trainStates: simulation.coaster_train_states(id),
      riders: simulation.coaster_riders(id),
    }));
    const coasterTransfers = coasters.flatMap(({ trainPoses, trainStates, riders }) => [trainPoses.buffer, trainStates.buffer, riders.buffer] as ArrayBuffer[]);
    sendWorkerMessage(outputScope,
      { type: "snapshot", sentAt: performance.now(), tickCount: Number(simulation.tick_count()), guestIds, guests, appearances, coasters },
      [guestIds.buffer, guests.buffer, appearances.buffer, ...coasterTransfers],
    );
  }
  const message = simulation.construction_message();
  const constructionEconomy = `${simulation.construction_coaster_price()}:${simulation.construction_coaster_income()}`;
  if (simulation.construction_active() && (message !== lastConstructionMessage || simulation.construction_block_report_json() !== lastBlockReport || constructionEconomy !== lastConstructionEconomy)) {
    sendConstruction();
    sendRideInfo();
  }
  if (sendCoasters() && selectedRide !== undefined) sendRideInfo();
  if (selectedGuestId !== undefined && now - lastGuestDetailSentAt >= 500) sendGuestDetail();
  if (simulation.facilities_json() !== lastFacilitiesJson || String(simulation.facility_revenue()) !== lastFacilityRevenue) sendFacilities();
  if (simulationPaused || gamePaused || !scheduleNext) return;
  setTimeout(wake, Math.max(0, clock.tickDurationMs - clock.accumulatorMs));
}

/** Publishes list changes and reports whether the selected ride's income changed. */
function sendCoasters(force = false): boolean {
  const json = simulation.coasters_json();
  if (!force && json === lastCoastersJson) return false;
  const coasters = coasterListEntries(
    JSON.parse(json) as CoasterListEntry[],
    JSON.parse(simulation.coaster_render_list_json()) as CoasterRenderManifest[],
  );
  const previousSelectedIncome = lastCoasters.find(({ id }) => id === selectedRide)?.income;
  const nextSelectedIncome = coasters.find(({ id }) => id === selectedRide)?.income;
  const selectedRideIncomeChanged = selectedRide !== undefined && previousSelectedIncome !== nextSelectedIncome;
  lastCoastersJson = json;
  lastCoasters = coasters;
  sendWorkerMessage(outputScope, { type: "coasters", coasters });
  return selectedRideIncomeChanged;
}

function sendGuestDetail(guestId = selectedGuestId): void {
  if (guestId === undefined) return;
  const detail = JSON.parse(simulation.guest_detail_json(guestId)) as GuestDetail | null;
  sendWorkerMessage(outputScope, { type: "guest-detail", guestId: `guest-${guestId}`, detail });
  lastGuestDetailSentAt = performance.now();
}

function sendRideInfo(testMessage = ""): void {
  if (selectedRide === undefined) return;
  const report = JSON.parse(simulation.coaster_ride_info_json(selectedRide)) as RideInfoReport;
  const coaster = selectedRide === DEMO_COASTER_ID
    ? undefined
    : (JSON.parse(simulation.coasters_json()) as CoasterListEntry[]).find(({ id }) => id === selectedRide);
  if (selectedRide !== DEMO_COASTER_ID && !coaster) throw new Error(`Ride info requested for missing coaster ${selectedRide}`);
  const info = {
    ...report,
    name: coaster?.name ?? report.modelName,
    open: coaster?.open ?? false,
    price: coaster?.price ?? 0,
    income: coaster?.income ?? 0,
    complete: coaster?.complete ?? true,
    editable: coaster !== undefined,
  } as RideInfo;
  sendWorkerMessage(outputScope, { type: "ride-info", info, testMessage });
}

type CoasterActionName = "open" | "price" | "edit" | "stop-editing" | "rename" | "demolish";

function sendCoasterActionResult(
  action: CoasterActionName,
  coasterId: number | null,
  success: boolean,
  message: string,
): void {
  sendWorkerMessage(outputScope, { type: "coaster-action-result", action, coasterId, success, message });
}

function readCoasterActionMessage(message: string, fallback: string): string {
  return message.trim() || fallback;
}

function coasterIdForRideAction(ride: unknown): number {
  if (typeof ride === "number" && Number.isInteger(ride) && ride >= 0 && ride <= 0xffff_ffff) return ride;
  if (ride === "demo") return DEMO_COASTER_ID;
  if (ride === "player") {
    const editingCoasterId = simulation.editing_coaster_id_value();
    if (editingCoasterId !== null && editingCoasterId !== undefined) return editingCoasterId;
    throw new Error("Legacy ride command refers to a player coaster, but no coaster is being edited");
  }
  throw new TypeError(`Invalid coaster ride ID: ${JSON.stringify(ride)}`);
}

function sendConstruction(complete = true): void {
  const estimates = readBuilderEstimates(simulation, builderSession, complete);
  const { constructionForcePreview, pieces, elements } = estimates;
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
  const layoutReport = JSON.parse(simulation.construction_train_layout()) as TrainLayout | null;
  const trainLayout = layoutReport ? parseTrainLayout(layoutReport) : null;
  const canAppend = simulation.construction_finish_preview_active() || simulation.construction_can_append();
  const appendReason = canAppend ? "" : simulation.construction_preview_reason();
  sendWorkerMessage(outputScope, { type: "construction-state", complete, active: simulation.construction_active(), chain: simulation.construction_chain(), colors: colorNames ? parseCoasterColors(colorNames) : null, trainLayout, pieces, elements,
    hillOptions: estimates.hillOptions,
    speedTurns: estimates.speedTurns,
    selectedShape: estimates.selectedShape,
    selected: selected || null, track,
    canAppend, appendReason, canClose: simulation.construction_can_close(),
    finishPreview: simulation.construction_finish_preview_active(),
    canFinish: simulation.construction_can_finish(), finishReason: simulation.construction_finish_reason(),
    finishOptions: JSON.parse(simulation.construction_finish_options_json()) as ConstructionFinishOptionsWire,
    testingReport: JSON.parse(simulation.construction_test_report_json()) as TestingReport | null,
    constructionForcePreview,
    candidateForcePreview: estimates.candidateForcePreview,
    speedMatch: estimates.speedMatch,
    canTest: simulation.construction_can_test(), isOpen: simulation.construction_is_open(), coasterPrice, coasterIncome, stationPositions: [...simulation.construction_station_positions()], message: lastConstructionMessage || selectedChoice?.refusal || "",
    blockReport: JSON.parse(lastBlockReport) as BlockReport, trainCount: simulation.construction_train_count(),
    minCarsPerTrain: simulation.construction_min_cars_per_train(), maxCarsPerTrain: simulation.construction_max_cars_per_train(),
    previewRails, previewTies, previewSupports, placedRails, placedTies, placedSupports, railDistances, blockCandidates,
  }, [previewRails.buffer, previewTies.buffer, previewSupports.buffer, placedRails.buffer, placedTies.buffer, placedSupports.buffer, railDistances.buffer, blockCandidates.buffer]);
}

function sendConstructionFinishStatus(): void {
  sendWorkerMessage(outputScope, {
    type: "construction-finish-status",
    canFinish: simulation.construction_can_finish(),
    finishReason: simulation.construction_finish_reason(),
  });
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
    const advance = (): void => {
      if (generation !== finishSearchGeneration) return;
      const deadline = performance.now() + finishSearchSliceMs;
      let complete = false;
      do {
        complete = simulation.construction_finish_search_advance();
      } while (!complete && performance.now() < deadline);
      if (complete) {
        finishSearchPending = false;
        sendConstructionFinishStatus();
        if (finishSearchAwaited) {
          finishSearchAwaited = false;
          sendWorkerMessage(outputScope, { type: "construction-finish-ready" });
        }
      }
      else setTimeout(advance, 0);
    };
    advance();
  }, 0);
}

function sendFacilities(message = "", placed = false, forced = false, outcome: PlacementOutcome | null = null): void {
  lastFacilitiesJson = simulation.facilities_json();
  const facilities = JSON.parse(lastFacilitiesJson) as Facility[];
  lastFacilityRevenue = String(simulation.facility_revenue());
  sendWorkerMessage(outputScope, { type: "facilities", facilities, message, placed, forced, revenue: lastFacilityRevenue, outcome });
}

function sendScenery(message = "", outcome: PlacementOutcome | null = null): void {
  const scenery = JSON.parse(simulation.scenery_json()) as Scenery[];
  sendWorkerMessage(outputScope, { type: "scenery", scenery, message, outcome });
}

function sendWorld(): void {
  const world = captureWorldSnapshot(simulation);
  const renderManifest = JSON.parse(simulation.coaster_render_list_json()) as CoasterRenderManifest[];
  const coasterTrackRevisionsAfterSend = new Map<number, string>();
  const transfers: Transferable[] = [...world.transfer];
  const coasters = renderManifest.map((coaster) => {
    const trackChanged = coasterTrackRevisions.get(coaster.id) !== coaster.trackRevision;
    const entry = {
      id: coaster.id,
      trackRevision: coaster.trackRevision,
      colors: coaster.colors ? parseCoasterColors(coaster.colors) : null,
      trainLayout: coaster.trainLayout ? parseTrainLayout(coaster.trainLayout) : null,
    };
    if (!trackChanged) {
      coasterTrackRevisionsAfterSend.set(coaster.id, coaster.trackRevision);
      return entry;
    }

    const supports = simulation.coaster_supports(coaster.id);
    const geometry = simulation.coaster_geometry(coaster.id);
    const rails = simulation.coaster_rails(coaster.id);
    const ties = simulation.coaster_ties(coaster.id);
    transfers.push(supports.buffer as ArrayBuffer, geometry.buffer as ArrayBuffer, rails.buffer as ArrayBuffer, ties.buffer as ArrayBuffer);
    coasterTrackRevisionsAfterSend.set(coaster.id, coaster.trackRevision);
    return { ...entry, supports, geometry, rails, ties };
  });
  sendWorkerMessage(outputScope, {
    ...world.message,
    editingCoasterId: simulation.editing_coaster_id_value() ?? null,
    coasters,
  }, transfers);
  coasterTrackRevisions = coasterTrackRevisionsAfterSend;
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
    testMode = action.frozen;
    simulationPaused = action.frozen;
    initialized = init().then(() => {
      try {
        if (action.park === "description") {
          simulation = WasmSimulation.from_park_description(action.seed, action.guestCount, action.descriptionJson);
          sessionRecordingAvailable = false;
        } else {
          simulation = new WasmSimulation(action.seed, action.guestCount, action.park);
          recorder = new SessionRecorder({ seed: action.seed.toString(), park: action.park, guestCount: action.guestCount });
          sessionRecordingAvailable = true;
        }
        builderSession = new BuilderSessionBridge();
        sendWorld();
        sendCoasters(true);
        sendFacilities();
        sendScenery();
        sendWorkerMessage(outputScope, { type: "piece-shapes", shapes: JSON.parse(piece_shapes_json()) as PieceShape[] });
        sendWorkerMessage(outputScope, { type: "ready" });
        lastWakeup = performance.now();
        wake(true);
      } catch (error) {
        publishInitializationError(error);
      }
    }, publishInitializationError);
    return;
  }
  if (!initialized) throw new Error("Worker must be initialized first");
  void initialized.then(() => {
    if (initializationError !== undefined) return;
    if (action.type === "construction-input-barrier") {
      sendWorkerMessage(outputScope, { type: "construction-input-barrier" });
      return;
    }
    if (action.type === "export-session") {
      if (sessionRecordingAvailable) sendWorkerMessage(outputScope, { type: "session-export", requestId: action.requestId, recording: sessionRecorder().snapshot(Number(simulation.tick_count())) });
      else sendWorkerMessage(outputScope, { type: "session-export", requestId: action.requestId, error: "Session export is unavailable after loading a park." });
      return;
    }
    if (action.type === "save-park") {
      try {
        const bytes = (simulation as WasmSimulation & { save_state(): Uint8Array }).save_state();
        const tickCount = Number(simulation.tick_count());
        sendWorkerMessage(outputScope, { type: "park-saved", requestId: action.requestId, bytes, tickCount }, [bytes.buffer]);
      } catch (error) {
        sendWorkerMessage(outputScope, { type: "park-saved", requestId: action.requestId, error: String(error) });
      }
      return;
    }
    if (action.type === "load-park") {
      let restored: WasmSimulation;
      try {
        restored = (WasmSimulation as typeof WasmSimulation & { load_state(bytes: Uint8Array): WasmSimulation }).load_state(action.bytes);
      } catch (error) {
        sendWorkerMessage(outputScope, { type: "park-loaded", requestId: action.requestId, error: String(error) });
        return;
      }
      const previousSimulation = simulation;
      const previousBuilderSession = builderSession;
      const previousClock = clock;
      const previousWakeup = lastWakeup;
      const previousCoasterTrackRevisions = coasterTrackRevisions;
      const previousConstructionMessage = lastConstructionMessage;
      const previousBlockReport = lastBlockReport;
      const previousConstructionEconomy = lastConstructionEconomy;
      const previousCoastersJson = lastCoastersJson;
      const previousCoasters = lastCoasters;
      const previousFacilitiesJson = lastFacilitiesJson;
      const previousFacilityRevenue = lastFacilityRevenue;
      const previousSelectedGuestId = selectedGuestId;
      const staged: { message: WorkerMessage; transfer?: Transferable[] }[] = [];
      simulation = restored;
      builderSession = new BuilderSessionBridge();
      selectedGuestId = undefined;
      outputScope = { postMessage(message, transfer) { staged.push({ message, transfer }); } };
      try {
        coasterTrackRevisions = new Map();
        lastConstructionMessage = "";
        lastBlockReport = "";
        lastConstructionEconomy = "";
        lastCoastersJson = "";
        lastCoasters = [];
        clock = createSimulationClock(tickRate, 4);
        sendWorld();
        sendCoasters(true);
        sendFacilities("", false, true);
        sendScenery();
        sendConstruction();
        lastWakeup = performance.now();
        wake(true, false);
      } catch (error) {
        outputScope = workerScope;
        simulation = previousSimulation;
        builderSession.free();
        builderSession = previousBuilderSession;
        restored.free();
        clock = previousClock;
        lastWakeup = previousWakeup;
        coasterTrackRevisions = previousCoasterTrackRevisions;
        lastConstructionMessage = previousConstructionMessage;
        lastBlockReport = previousBlockReport;
        lastConstructionEconomy = previousConstructionEconomy;
        lastCoastersJson = previousCoastersJson;
        lastCoasters = previousCoasters;
        lastFacilitiesJson = previousFacilitiesJson;
        lastFacilityRevenue = previousFacilityRevenue;
        selectedGuestId = previousSelectedGuestId;
        sendWorkerMessage(outputScope, { type: "park-loaded", requestId: action.requestId, error: String(error) });
        return;
      }
      outputScope = workerScope;
      previousSimulation.free();
      previousBuilderSession.free();
      sessionRecordingAvailable = false;
      finishSearchGeneration += 1;
      finishSearchPending = false;
      finishSearchAwaited = false;
      selectedGuestId = undefined;
      selectedRide = undefined;
      sendWorkerMessage(outputScope, { type: "park-restoring" });
      for (const item of staged) sendWorkerMessage(outputScope, item.message, item.transfer);
      sendWorkerMessage(outputScope, { type: "park-loaded", requestId: action.requestId });
      // Loading discards saved finish results, so search again for the loaded track.
      scheduleFinishSearch();
      scheduleBlockAnalysis();
      return;
    }
    if (action.type === "construction-finish-await") {
      if (finishSearchPending) finishSearchAwaited = true;
      else sendWorkerMessage(outputScope, { type: "construction-finish-ready" });
      return;
    }
    if (shouldInvalidateConstructionFinishSearch(action)) {
      simulation.construction_finish_invalidate();
      scheduleFinishSearch();
    }
    if (!["test-resume", "test-advance", "test-advance-block-analysis", "test-advance-block-placement", "path-preview", "facility-access-preview", "guest-detail-request", "guest-detail-close", "ride-info-request", "ride-info-close", "construction-delete-mode"].includes(action.type)) {
      recordSessionCommand(action.type === "ride-test" ? { ...action, ride: coasterIdForRideAction(action.ride) } : action);
    }
    if (action.type === "test-resume") {
      if (!simulationPaused || !manualBlockAnalysis) throw new Error("Test resume requires a frozen worker with manual block analysis");
      simulationPaused = false;
      gamePaused = false;
      manualBlockAnalysis = false;
      clock = createSimulationClock(tickRate, 4);
      lastWakeup = performance.now();
      sendWorkerMessage(outputScope, { type: "test-resumed" });
      wake(true);
      scheduleBlockAnalysis();
      return;
    }
    if (action.type === "set-speed") {
      if (!Number.isInteger(action.speed) || action.speed < 1 || action.speed > 16) throw new RangeError("Invalid game speed");
      gameSpeed = action.speed;
      return;
    }
    if (action.type === "set-paused") {
      gamePaused = action.paused;
      lastWakeup = performance.now();
      if (!gamePaused) clock = createSimulationClock(tickRate, 4);
      wake(true);
      return;
    }
    if (action.type === "test-advance-block-analysis") {
      if (!manualBlockAnalysis) throw new Error("Test block analysis advancement is disabled");
      validateBlockAnalysisBudget(action.budget);
      advanceBlockAnalysis(action.budget);
      sendWorkerMessage(outputScope, { type: "test-block-analysis-advanced" });
      return;
    }
    if (action.type === "test-advance-block-placement") {
      if (!testMode || !manualBlockAnalysis) throw new Error("Test block placement advancement is disabled");
      validateBlockAnalysisBudget(action.budget);
      recordSessionCommand(action);
      simulation.construction_advance_block_placement_for_test(action.budget);
      sendBlockAnalysisProgress();
      sendWorkerMessage(workerScope, { type: "test-block-analysis-advanced" });
      return;
    }
    if (action.type === "test-advance") {
      if (!testMode || (!simulationPaused && !gamePaused)) throw new Error("Test tick advance requires a paused test simulation");
      if (!Number.isInteger(action.count) || action.count < 0 || action.count > 100000) throw new RangeError("Invalid test tick count");
      simulation.test_begin_queue_entry_capture();
      for (let tick = 0; tick < action.count; tick++) tickSimulation();
      const queueEntryPlaceIds = JSON.parse(simulation.test_take_queue_entry_places_json()) as number[];
      wake(true);
      if (selectedGuestId !== undefined) sendGuestDetail();
      sendFacilities("", false, true);
      sendWorkerMessage(outputScope, {
        type: "test-advanced",
        queueEntryPlaceIds,
      });
      return;
    }
    if (action.type === "scenery-place" || action.type === "scenery-remove") {
      const success = action.type === "scenery-place"
        ? simulation.place_scenery(action.kind, action.x, action.y, action.rotation, action.quarter)
        : simulation.remove_scenery(action.id);
      if (success) sendWorld();
      const outcome = success ? (action.type === "scenery-place" ? "placed" : "removed") : "refused";
      sendScenery(simulation.scenery_message() || (success ? (action.type === "scenery-place" ? "Scenery placed." : "Scenery removed.") : "Scenery could not be changed."), outcome);
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
      sendFacilities(removed ? "Facility removed." : simulation.facility_message(), false, false, removed ? "removed" : "refused");
      if (simulation.construction_active()) sendConstruction();
      return;
    }
    if (action.type === "facility-access-preview") {
      const rotation = simulation.facility_access_rotation(action.kind, action.x, action.y, action.height, action.rotation);
      sendWorkerMessage(outputScope, { type: "facility-access-preview", requestId: action.requestId, rotation: rotation < 0 ? undefined : rotation });
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
      sendFacilities(placed ? "Facility placed." : simulation.facility_message(), placed, false, placed ? "placed" : "refused");
      if (simulation.construction_active()) sendConstruction();
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
      }
      if (simulation.construction_active()) sendConstruction();
      const invalidatedPaths = simulation.terrain_invalidated_paths();
      sendWorkerMessage(outputScope, { type: "terrain-result", success, message: simulation.terrain_message(), invalidatedPaths }, [invalidatedPaths.buffer]);
      return;
    }
    if (action.type === "guest-detail-request") {
      const match = /^guest-(\d+)$/.exec(action.guestId);
      if (!match) {
        sendWorkerMessage(outputScope, { type: "guest-detail", guestId: action.guestId, detail: null });
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
    if (action.type === "ride-info-request") {
      selectedRide = coasterIdForRideAction(action.ride);
      sendRideInfo();
      return;
    }
    if (action.type === "ride-info-close") {
      selectedRide = undefined;
      return;
    }
    if (action.type === "ride-test") {
      const coasterId = coasterIdForRideAction(action.ride);
      selectedRide = coasterId;
      const testMessage = simulation.coaster_test(coasterId);
      sendRideInfo(testMessage);
      return;
    }
    if (action.type === "ride-set-open") {
      const coasterId = coasterIdForRideAction(action.ride);
      const validTarget = coasterId !== DEMO_COASTER_ID;
      const success = validTarget && simulation.coaster_set_open(coasterId, action.open);
      const message = success
        ? (action.open ? "Ride opened." : "Ride closed.")
        : validTarget
          ? readCoasterActionMessage(simulation.construction_message(), "Ride could not be changed.")
          : "The demo coaster cannot be changed.";
      sendCoasters(true);
      if (simulation.editing_coaster_id_value() === coasterId) sendConstruction();
      if (success) sendWorld();
      sendRideInfo();
      sendCoasterActionResult("open", coasterId, success, message);
      return;
    }
    if (action.type === "ride-set-price") {
      const coasterId = coasterIdForRideAction(action.ride);
      const validPrice = Number.isInteger(action.price) && action.price >= 0 && action.price <= 65_535;
      const validTarget = coasterId !== DEMO_COASTER_ID;
      const success = validTarget && validPrice && simulation.coaster_set_price(coasterId, action.price);
      const message = success
        ? "Ticket price updated."
        : !validTarget
          ? "The demo coaster cannot be changed."
          : !validPrice
            ? "Ticket price must be an integer between 0 and 65,535."
            : "Ticket price could not be changed.";
      sendCoasters(true);
      if (simulation.editing_coaster_id_value() === coasterId) sendConstruction();
      sendRideInfo();
      sendCoasterActionResult("price", coasterId, success, message);
      return;
    }
    if (action.type === "ride-edit") {
      const coasterId = coasterIdForRideAction(action.ride);
      const validTarget = coasterId !== DEMO_COASTER_ID;
      const success = validTarget && simulation.coaster_edit(coasterId);
      if (success) {
        builderSession.invalidate();
        sendWorld();
        sendCoasters(true);
        sendConstruction();
        sendRideInfo();
        scheduleFinishSearch();
        scheduleBlockAnalysis();
      }
      sendCoasterActionResult("edit", coasterId, success, success ? "Editing coaster." : validTarget ? "Coaster could not be edited." : "The demo coaster cannot be edited.");
      return;
    }
    if (action.type === "ride-stop-editing") {
      const coasterId = simulation.editing_coaster_id_value() ?? null;
      builderSession.invalidate();
      simulation.coaster_stop_editing();
      sendWorld();
      sendConstruction();
      sendCoasterActionResult("stop-editing", coasterId, true, "Stopped editing the coaster.");
      return;
    }
    if (action.type === "ride-rename") {
      const coasterId = coasterIdForRideAction(action.ride);
      const validTarget = coasterId !== DEMO_COASTER_ID;
      const validName = action.name.trim().length > 0;
      const success = validTarget && validName && simulation.coaster_rename(coasterId, action.name);
      const message = success
        ? "Coaster renamed."
        : !validTarget
          ? "The demo coaster cannot be renamed."
          : !validName
            ? "Coaster name cannot be empty."
            : "Coaster could not be renamed.";
      sendCoasters(true);
      sendRideInfo();
      sendCoasterActionResult("rename", coasterId, success, message);
      return;
    }
    if (action.type === "ride-demolish") {
      const coasterId = coasterIdForRideAction(action.ride);
      const validTarget = coasterId !== DEMO_COASTER_ID;
      const success = validTarget && simulation.coaster_demolish(coasterId);
      if (success) {
        if (selectedRide === coasterId) selectedRide = undefined;
        sendWorld();
        sendCoasters(true);
        sendConstruction();
        if (selectedRide !== undefined) sendRideInfo();
      }
      const message = success ? "Coaster demolished." : validTarget ? "Coaster could not be demolished." : "The demo coaster cannot be demolished.";
      sendCoasterActionResult("demolish", coasterId, success, message);
      return;
    }
    if (action.type === "path-preview") {
      validatePathRoute(action);
      const paths = simulation.path_preview(
        pathRouteCoordinates(action.start, action.end),
        action.kind,
        action.style,
      );
      sendWorkerMessage(outputScope,
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
      sendWorkerMessage(outputScope, {
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
      sendWorkerMessage(outputScope, {
        type: "path-command-result",
        requestId: action.requestId,
        command: "remove",
        success,
        message: simulation.path_message() || (success ? "Path removed." : "Path removal failed."),
      });
      return;
    }
    if (action.type !== "set-surface") {
      if (!applyConstructionAction(simulation, builderSession, action, scheduleFinishSearch)) return;
      if (sendsIncompleteConstructionState(action, simulation)) sendConstruction(false);
      if (["construction-start", "construction-append", "construction-append-resolved", "construction-undo", "construction-delete", "construction-close", "construction-finish-confirm", "construction-set-open", "construction-set-colors", "construction-set-cars-per-train"].includes(action.type)) sendWorld();
      sendConstruction();
      sendCoasters();
      sendRideInfo();
      scheduleBlockAnalysis();
      return;
    }
    validateSurfaceEdit(action, simulation.world_dimensions());
    const { x, y, corners, water } = action;
    const changed = simulation.set_world_surface(x, y, corners, water);
    if (changed) {
      sendWorld();
    }
    if (simulation.construction_active()) sendConstruction();
    sendWorkerMessage(outputScope, { type: "surface-edited", changed });
  });
});
