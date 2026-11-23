import { createParkSaves } from "./park-saves";
import { canvasBackingSizeForBounds } from "./canvas-sizing";
import { landscapingTileFromPick, sceneryTileFromPick, entityIdFromPick, facilityIdFromPick, blockBoundaryIndexFromPick, facilityTileFromPicks, guestSelectionActionFromPick, stationTileFromSurfacePick } from "./world-picking";
import { createStationTool } from "./station-tool";
import { createFacilityTool } from "./facility-tool";
import { stationToolAdapter, facilityToolAdapter, pathToolAdapter, landscapingToolAdapter, sceneryToolAdapter, coasterToolAdapter } from "./tool-adapters";
import { createRenderRevision, shouldSkipRenderFrame } from "./render-revision";
import { composeScene } from "./compose-scene";
import { sendWorkerAction as postWorkerAction, assertNever, type WorkerAction, type WorkerMessage, type CoasterListEntry, type CoasterSnapshot, type CoasterRenderData } from "./protocol";
import { createAutosaveScheduler, getAutosaveSettings, type AutosaveScheduler } from "./autosave";
import "./print-modern.css";
import "./app.css";
import type { Facility } from "./facilities";
import { createFacilityPanel } from "./facility-panel";
import { applyCameraAction, projectWorld, type Camera, type Viewport, type WorldPoint } from "./camera";
import { bindInput, bindKeyboard, builderShortcuts, keyHint } from "./input/bindings";
import type { CameraAction, InputAction, ScreenPoint, UndergroundStyle } from "./input/actions";
import { constructionRiderPositions, readConstructionEstimates, validateBlockAnalysisBudget, type FrameLogEntry, type GameTest, type SnapshotArrivalEntry } from "./game-test";
import { blockLayoutChanged } from "./block-analysis-progress";
import { createWorldRenderer, loadAtlas, type Cutaway, type WorldView, type WorldSprite } from "./render/index";
import { CUTAWAY_RADIUS, cutawayLevelForTrack, insideCutaway } from "./render/cutaway";
import { advanceDayPhase, NOON, START_PHASE } from "./render/daylight";
import { CONFETTI_LIFE_TICKS, createCrestTracker, detectCrest, type CrestBurst } from "./render/effects";
import { createDayClock, type DaySpeed } from "./day-clock";
import { fitGameBar } from "./game-bar-fit";
import { trackStyle } from "./render/track/geometry";
import { repaint } from "./coaster-colors";
import { steppedTrainCount } from "./block-panel-state";
import { createStaticTrackCache } from "./render/track/cache";
import { RenderClock } from "./render-clock";
import { createConstructionPanel } from "./construction";
import { createCoasterList } from "./coaster-list";
import { createConstructionInputQueue } from "./construction-input-queue";
import type { BlockAnalysisProgress, ConstructionState, PieceShapesMessage } from "./construction-protocol";
import { followTrackEnd, railPoints } from "./construction-camera";
import { createConstructionEffects } from "./construction-effects";
import { nearestBlockBoundaryMetres } from "./render/track/blocks";
import { createGuestPanel } from "./guest-panel/guest-panel";
import { createRideWindow } from "./ride-window/ride-window";
import { rideFromPick } from "./ride-window/picking";
import type { RideInfo } from "./ride-window/protocol";
import type { GuestDetail } from "./guest-panel/protocol";
import { createPathTool, type PathTile, type PathToolMessage } from "./path-tool";
import { createPathPanel } from "./path-panel";
import { pathTileFromPick } from "./path-picking";
import { decodePathPreview, pathPreviewRefusalAt } from "./render/path-ghost";
import { createTestingPanel, type TestingMetric } from "./coaster-testing";
import { createLandscapingTool, type TerrainEditResult } from "./landscaping-tool";
import { createLandscapingPanel } from "./landscaping-panel";
import { createSceneryTool, type Scenery } from "./scenery-tool";
import { createSceneryPanel } from "./scenery-panel";
import { createToolbar, type Tool } from "./toolbar";
import { createThemeToggle } from "./theme";
import type { WorkerRecording } from "./session-recording";
import { parseReplaySession, type ReplaySession } from "./session-replay";
import { createGamePauseState } from "./game-pause";
import { replaySessionIntoWorker } from "./session-replay-worker";
import { constructionHeights, type HeightExtremum, type HeightMarker } from "./height-markers";
import { createForceOverlay, nearestSampleIndex, type ScreenBox } from "./force-overlay";
import { formatLength, type Units } from "./units";
import { createUnitsSetting } from "./units-setting";
import { createLiveAudio, renderOffline } from "./audio/engine";
import { soundFrame, type MappingMemory, type UiEvent } from "./audio/mapping";
import { constructionUiEvents, createUiEventQueue, pathUiEvent, placementUiEvent, terrainUiEvent } from "./audio/ui-events";
import { createSoundSetting } from "./sound-setting";
import { parseTrainStates, validateTrainStates } from "./train-states";
import { bundledParkDescriptionJson } from "./park-bundle";
import {
  constructionCutawayTarget,
  landscapingCutawayTarget,
  pathCutawayTarget,
  pathTileCutawayTarget,
  shouldShowAutomaticCutaway,
  storeUndergroundCutawayMode,
  storedUndergroundCutawayMode,
  type UndergroundCutawayTarget,
} from "./underground-cutaway";

declare const __GIT_HASH__: string;

let autosaveScheduler: AutosaveScheduler | undefined;

function sendWorkerAction(worker: Worker, action: WorkerAction): void {
  postWorkerAction(worker, action);
  if (isParkMutationIntent(action)) autosaveScheduler?.markChanged();
}

function isParkMutationIntent(action: WorkerAction): boolean {
  switch (action.type) {
    case "construction-start":
    case "construction-select":
    case "construction-set-chain":
    case "construction-set-launch-intensity":
    case "construction-append":
    case "construction-append-resolved":
    case "construction-undo":
    case "construction-delete":
    case "construction-close":
    case "construction-finish-preview":
    case "construction-finish-confirm":
    case "construction-finish-cycle":
    case "construction-finish-cancel":
    case "construction-set-open":
    case "construction-set-coaster-price":
    case "construction-set-colors":
    case "construction-test":
    case "construction-auto-blocks":
    case "construction-apply-best-blocks":
    case "construction-set-train-count":
    case "construction-set-cars-per-train":
    case "construction-move-block":
    case "construction-remove-block":
    case "construction-add-block":
    case "construction-set-block-sections":
    case "ride-edit":
    case "ride-set-open":
    case "ride-set-price":
    case "ride-rename":
    case "ride-demolish":
      return true;
    default:
      return false;
  }
}

interface RecordedInput { action: InputAction; time: number; tick: number; viewport: Viewport; camera: Camera }
const renderRevision = createRenderRevision();
const inputLog: RecordedInput[] = [];
let sessionExportRequest = 0;
const pendingSessionExports = new Map<number, RecordedInput[]>();
const coasterActions = new Set<InputAction["type"]>(["piece-step", "piece-group", "delete-mode-toggle", "chain-toggle"]);

const themeToggleContainer = document.querySelector<HTMLElement>("[data-theme-toggle]");
if (!themeToggleContainer) throw new Error("Theme toggle is missing");
// Created before the atlas loads, so a stored choice applies as early as possible.
const themeToggle = createThemeToggle(themeToggleContainer, dispatch, () => { renderRevision.invalidate(); });
const canvas = document.querySelector<HTMLCanvasElement>("canvas");
const tickOutput = document.querySelector<HTMLOutputElement>("[data-testid='tick-count']");
if (!canvas || !tickOutput) throw new Error("Game page elements are missing");
const gameCanvas = canvas;
const output = tickOutput;
/** The canvas in backing-store pixels, where pointer positions, picking and drawing happen. */
let viewport: Viewport = { width: gameCanvas.width, height: gameCanvas.height };
let pixelRatio = 1;
/** The camera's zoom is in CSS pixels, so the park keeps its size on screens of any pixel density. */
let camera: Camera = { centerX: 250, centerY: 256, zoom: 1, rotation: 0 };
const cssViewport = (): Viewport => ({ width: viewport.width / pixelRatio, height: viewport.height / pixelRatio });
const screenCamera = (): Camera => ({ ...camera, zoom: camera.zoom * pixelRatio });
function moveCamera(action: CameraAction): void {
  const cssAction = action.type === "pan" ? { ...action, dx: action.dx / pixelRatio, dy: action.dy / pixelRatio }
    : action.type === "zoom" ? { ...action, anchor: { x: action.anchor.x / pixelRatio, y: action.anchor.y / pixelRatio } } : action;
  camera = applyCameraAction(camera, cssViewport(), cssAction);
}
type CoasterWorld = WorldView & { pathMetadata: Int32Array; editingCoasterId: number | null; coasters: readonly CoasterRenderData[] };
let world: CoasterWorld | undefined;
let previous: CoasterSnapshot | undefined;
let current: CoasterSnapshot | undefined;
let coasterQueueLengths: readonly Pick<CoasterListEntry, "id" | "queueLength">[] = [];
const renderClock = new RenderClock<CoasterSnapshot>(30);
const testMode = new URLSearchParams(location.search).has("test");
const livePauseRender = new URLSearchParams(location.search).get("livePauseRender") === "1";
let frozen = testMode;
const frameLogEnabled = new URLSearchParams(location.search).get("frameLog") === "1";
const frameLogEntries: FrameLogEntry[] = [];
const snapshotArrivalEntries: SnapshotArrivalEntry[] = [];
let manualBlockAnalysis = frozen && new URLSearchParams(location.search).has("manualBlockAnalysis");
const testAgents = frozen ? new URLSearchParams(location.search).get("agents") : null;
const timing = new URLSearchParams(location.search).get("timing") === "work";
const { atlas, images } = await loadAtlas(new URL("./atlas.json", document.baseURI).href);
const renderer = createWorldRenderer(gameCanvas, atlas, images, frozen);
const staticTrack = createStaticTrackCache(atlas);
const builtTrack = createStaticTrackCache(atlas);
let construction: ConstructionState | undefined;
let selectedBlock: number | null = null;
function blockAt(position: { x: number; y: number }): number | null {
  if (activeTool !== "coaster") return null;
  const pick = renderer.pick(position.x, position.y);
  return blockBoundaryIndexFromPick(pick);
}
function selectBlock(index: number | null): void {
  selectedBlock = index;
  panel.setSelectedBlock(index);
  renderRevision.invalidate();
}
let deleteMode = false;
const stationTool = createStationTool((command) => sendWorkerAction(worker, command), () => {
  const station = stationTool.state().station;
  gameCanvas.dataset.stationPreview = station ? `${station.x},${station.y},${station.z},${station.heading}` : "";
  renderRevision.invalidate();
});
const stationAdapter = stationToolAdapter(stationTool);
const panelContainer = document.querySelector<HTMLElement>("[data-construction]");
if (!panelContainer) throw new Error("Construction panel is missing");
const coasterToolContainer = document.querySelector<HTMLElement>("[data-coaster-tool]");
const coasterListContainer = document.querySelector<HTMLElement>("[data-coaster-list]");
if (!coasterToolContainer || !coasterListContainer) throw new Error("Coaster tool is missing");
const coasterList = createCoasterList(coasterListContainer, {
  onSelect(id) { dispatch({ type: "ride-select", ride: id }); },
  onNewCoaster() { sendWorkerAction(worker, { type: "ride-stop-editing" }); showCoasterBuilder(); },
});
function showCoasterBuilder(): void {
  coasterListContainer!.hidden = true;
  panelContainer!.hidden = false;
}
const panel = createConstructionPanel(panelContainer, dispatch, () => {
  const id = world?.editingCoasterId;
  if (id !== null && id !== undefined) dispatch({ type: "ride-select", ride: id });
});
// The whole page, so builder keys keep working after a click in the panel or when a re-rendered control drops focus.
bindKeyboard(document.body, dispatch, () => ({ x: gameCanvas.width / 2, y: gameCanvas.height / 2 }), () => activeTool === "coaster" && selectedBlock !== null);
const constructionEffects = createConstructionEffects(trackStyle(atlas));
const uiEvents = createUiEventQueue();
/** Builder and tool sounds follow the player's own actions, so frozen test mode and session replays queue none. */
function queueUiEvent(event: UiEvent | null): void {
  if (event && !frozen && !replayInProgress) uiEvents.push(event, performance.now());
}
let followPoints: WorldPoint[] | undefined;
let followedPreview = "";
let lastFrameAt = performance.now();
let testingMetric: TestingMetric | null = null;
let testingReportJson = "";
let constructionForcePreviewJson = "";
const testingPanel = createTestingPanel(panelContainer.querySelector<HTMLElement>("[data-testing]")!, trackStyle(atlas), dispatch, () => renderRevision.invalidate());
const forceOverlay = createForceOverlay(document.querySelector<HTMLElement>("[data-force-overlay]")!);
/** Hover reaches this many CSS pixels from the coloured track. */
const FORCE_HOVER_PIXELS = 14;
if (!atlas.themeStyle) throw new Error("Atlas theme style is missing");
let selectedGuestId: string | undefined;
const guestPanel = createGuestPanel(gameCanvas, { dispatch });
let selectedRide: number | undefined;
let latestRideInfoJson = "";
let latestRideInfo: RideInfo | null = null;
const rideWindow = createRideWindow(gameCanvas, { dispatch, onHighlightChange: () => renderRevision.invalidate() });

let facilities: Facility[] = [];
let facilityPanelSnapshot = "";
const facilityTool = createFacilityTool((command) => sendWorkerAction(worker, command), () => {
  const message = facilityTool.state().message;
  if (message) facilityPanel.message(message);
});
const facilityAdapter = facilityToolAdapter(facilityTool);
let advanceTicksComplete: ((result: {
  readonly queueEntryPlaceIds: readonly number[];
}) => void) | undefined;
let advanceBlockAnalysisComplete: (() => void) | undefined;
const facilityContainer = document.querySelector<HTMLElement>("[data-facilities]");
if (!facilityContainer) throw new Error("Facility panel is missing");
const facilityPanel = createFacilityPanel(facilityContainer, { atlas, images }, dispatch);
const pathContainer = document.querySelector<HTMLElement>("[data-paths]");
if (!pathContainer) throw new Error("Path panel is missing");
const pathsElement = pathContainer;
let pathHover: PathTile | null = null;
const pathTool = createPathTool((command) => sendWorkerAction(worker, command), () => {
  const state = pathTool.state();
  const refusal = state.preview && pathHover ? pathPreviewRefusalAt(decodePathPreview(state.preview.paths), pathHover.x, pathHover.y) : null;
  pathPanel.message(refusal ?? state.message);
  renderRevision.invalidate();
});
const pathAdapter = pathToolAdapter(pathTool);
const pathPanel = createPathPanel(pathContainer, dispatch);
pathContainer.querySelector<HTMLElement>("[data-build]")!.title = `Build paths (${keyHint(builderShortcuts.pathBuild)})`;
pathContainer.querySelector<HTMLElement>("[data-remove]")!.title = `Remove paths (${keyHint(builderShortcuts.pathRemove)})`;
let manualUndergroundEnabled = false;
let selectedUndergroundLevel = 8;
let undergroundStyle: UndergroundStyle = "cutaway";
let undergroundCutawayMode = storedUndergroundCutawayMode();
let lastLandscapingUndergroundEnabled = false;
/** The cutaway window drawn in the last frame; picking inside it uses its cut level. */
let renderedCutaway: Cutaway | undefined;
let pointerPosition: ScreenPoint | null = null;
const landscapingContainer = document.querySelector<HTMLElement>("[data-landscaping]");
if (!landscapingContainer) throw new Error("Landscaping panel is missing");
const landscapingPanel = createLandscapingPanel(landscapingContainer, dispatch);
landscapingPanel.setUndergroundView(false, selectedUndergroundLevel, undergroundStyle);
panel.setUndergroundCutawayMode(undergroundCutawayMode);
const landscapingTool = createLandscapingTool((command) => {
  if (!world || command.x < 0 || command.y < 0 || command.x + command.size > world.dimensions[0] || command.y + command.size > world.dimensions[1]) {
    landscapingPanel.message("Brush extends outside the park.");
    queueUiEvent({ sound: "error" });
    return;
  }
  sendWorkerAction(worker, command);
}, () => { renderRevision.invalidate(); });

const landscapingAdapter = landscapingToolAdapter(landscapingTool);

let scenery: Scenery[] = [];
const sceneryContainer = document.querySelector<HTMLElement>("[data-scenery]");
if (!sceneryContainer) throw new Error("Scenery panel is missing");
const sceneryTool = createSceneryTool((command) => sendWorkerAction(worker, command), () => {
  sceneryPanel.message(sceneryTool.state().message);
  renderRevision.invalidate();
});
const sceneryAdapter = sceneryToolAdapter(sceneryTool);
const sceneryPanel = createSceneryPanel(sceneryContainer, { atlas, images }, dispatch);

const dock = document.querySelector<HTMLElement>("[data-dock]");
const toolbarContainer = document.querySelector<HTMLElement>("[data-toolbar]");
if (!dock || !toolbarContainer) throw new Error("Tool dock or toolbar is missing");
const toolDock = dock;
const tunnelPreview = document.querySelector<HTMLElement>("[data-tunnel-preview]")!;
const toolPanels: Record<Exclude<Tool, "none">, HTMLElement> = { coaster: coasterToolContainer, paths: pathContainer, landscape: landscapingContainer, facilities: facilityContainer, scenery: sceneryContainer };
const toolbar = createToolbar(toolbarContainer, (tool) => dispatch({ type: "tool-select", tool }));
let activeTool: Tool = "none";
/** The time of day follows simulation ticks, so it pauses with the game. Test mode holds it at noon unless a test sets it. */
let dayPhase = frozen ? NOON : START_PHASE;
let daySpeed: DaySpeed = 1;
const gamePause = createGamePauseState();
let gameSpeed = 1;
let dayTick: number | undefined;
let coasterCrests = new Map<number, ReturnType<typeof createCrestTracker>>();
let crestBursts: CrestBurst[] = [];
let soundMemory: MappingMemory | undefined;
/** Every crest burst since the page loaded, for tests. */
const crestHistory: CrestBurst[] = [];
function setTimeOfDay(phase: number): void {
  dayPhase = advanceDayPhase(phase, 0, 1);
  renderRevision.invalidate();
}
const dayClockContainer = document.querySelector<HTMLElement>("[data-day-clock]");
if (!dayClockContainer) throw new Error("Day clock is missing");
const dayClock = createDayClock(dayClockContainer, dispatch);
const gameBar = document.querySelector<HTMLElement>(".game-bar");
if (!gameBar) throw new Error("Game bar is missing");
fitGameBar(gameBar);
dayClock.setPaused(gamePause.isPaused());
/** Size the canvas backing store to its displayed size in device pixels, so the park is drawn sharp at any pixel ratio. */
function fitCanvas(keepCorner = true): void {
  const bounds = gameCanvas.getBoundingClientRect();
  const size = canvasBackingSizeForBounds(bounds, devicePixelRatio);
  if (!size || (size.width === gameCanvas.width && size.height === gameCanvas.height)) return;
  const previous = cssViewport();
  gameCanvas.width = size.width;
  gameCanvas.height = size.height;
  viewport = { width: size.width, height: size.height };
  pixelRatio = size.pixelRatio;
  // Keep the top-left corner of the park still, so opening a tool covers the right of the view instead of sliding it.
  if (keepCorner) camera = applyCameraAction(camera, cssViewport(), { type: "pan", dx: (previous.width - bounds.width) / 2, dy: (previous.height - bounds.height) / 2 });
  renderRevision.invalidate();
}
function watchPixelRatio(): void {
  matchMedia(`(resolution: ${devicePixelRatio}dppx)`).addEventListener("change", () => { fitCanvas(); watchPixelRatio(); }, { once: true });
}
new ResizeObserver(() => fitCanvas()).observe(gameCanvas);
watchPixelRatio();
function inputModeActive(): boolean {
  return sceneryTool.state().mode !== "inactive" || stationTool.state().active || deleteMode || facilityTool.state().mode !== "inactive" || pathTool.state().mode !== "inactive" || landscapingTool.state().active;
}
function endInputModes(): void {
  if (sceneryTool.state().mode !== "inactive") sceneryAdapter.deactivate();
  if (landscapingTool.state().active) landscapingAdapter.deactivate();
  if (pathTool.state().mode !== "inactive") pathAdapter.deactivate();
  facilityAdapter.deactivate();
  facilityPanel.previewDirection(undefined);
  deleteMode = false; panel.setDeleteMode(false);
  if (stationTool.state().active) dispatch({ type: "station-placement", enabled: false });
}
/** Every input mode belongs to one tool, so opening another tool ends it and only the open tool's mode can be active. */
function openTool(tool: Tool, activatePanel = true): void {
  if (tool === activeTool) return;
  if (activeTool === "coaster") {
    constructionInputQueue.cancel();
    sendWorkerAction(worker, { type: "ride-stop-editing" });
  }
  endInputModes();
  activeTool = tool;
  renderRevision.invalidate();
  if (tool === "coaster") coasterAdapter.activate(undefined); else coasterAdapter.deactivate();
  toolbar.setActive(tool);
  dayClock.setControlsVisible(tool === "none");
  toolDock.hidden = tool === "none";
  for (const [owner, element] of Object.entries(toolPanels)) element.hidden = owner !== tool;
  if (tool === "coaster") { coasterListContainer!.hidden = false; panelContainer!.hidden = true; }
  tunnelPreview.hidden = tool !== "coaster" || tunnelPreview.textContent === "";
  toolDock.scrollTop = 0;
  // Resize now rather than on the next observer callback, so the world is projected onto the new canvas size at once.
  fitCanvas();
  if (activatePanel && tool === "paths") pathPanel.activate();
  else if (activatePanel && tool === "landscape") landscapingPanel.activate();
  else if (activatePanel && tool === "scenery") sceneryPanel.activate();
  else if (activatePanel && tool === "facilities") facilityPanel.activate();
}
let latestGuestDetail: GuestDetail | null = null;
let renderedGuests: readonly WorldSprite[] = [];
let renderedRiders: { id: string; left: number; right: number; top: number; bottom: number }[] = [];
const gameTest: GameTest = {
  audio: { state: () => audio.state(), unlock: () => audio.unlock(), activeVoices: () => audio.activeVoices(), lastUiEvent: () => audio.lastUiEvent(), renderOffline },
  get ready() { return renderRevision.ready(); },
  tickCount() { return current?.tickCount ?? 0; },
  frameLog() { return frameLogEnabled ? frameLogEntries.map((entry) => ({ ...entry })) : []; },
  snapshotArrivals() { return frameLogEnabled ? snapshotArrivalEntries.map((entry) => ({ ...entry })) : []; },
  guests() { return renderedGuests; },
  renderedRiders() { return renderedRiders; },
  riders() {
    return current?.coasters.flatMap(({ trainPoses, riders }) => constructionRiderPositions(trainPoses, riders)) ?? [];
  },
  facilities() { return facilities; },
  paths() {
    if (!world) return [];
    const paths = [];
    for (let offset = 0; offset < world.paths.length; offset += 4) {
      const [x, y, z, slope] = world.paths.subarray(offset, offset + 4);
      const [kind, style, edges] = world.pathMetadata.subarray(offset / 4 * 3, offset / 4 * 3 + 3);
      paths.push({ x, y, z, slope, kind, style, edges });
    }
    return paths;
  },
  pathPreview() {
    const preview = pathTool.state().preview;
    return preview ? decodePathPreview(preview.paths).map(({ x, y, edges }) => ({ x, y, edges })) : [];
  },
  carvedSpans() {
    if (!world) return [];
    if (!world.carved) throw new Error("Carved world spans are missing");
    const spans: [number, number, number, number][] = [];
    for (let offset = 0; offset < world.carved.length; offset += 4) {
      spans.push([world.carved[offset]!, world.carved[offset + 1]!, world.carved[offset + 2]!, world.carved[offset + 3]!]);
    }
    return spans;
  },
  requestGuestDetail(id) { sendWorkerAction(worker, { type: "guest-detail-request", guestId: id }); },
  guestDetail() { return latestGuestDetail; },
  rideInfo() { return latestRideInfo; },
  testingReport() { return construction?.testingReport ?? null; },
  constructionEstimates() { return readConstructionEstimates(construction); },
  constructionForcePreview() { return construction?.constructionForcePreview ?? null; },
  constructionTrainPoses() { return Array.from(current?.coasters.find(({ id }) => id === world?.editingCoasterId)?.trainPoses ?? []); },
  coasterQueueLengths() { return coasterQueueLengths; },
  coasters() { return current?.coasters ?? []; },
  editingCoasterId() { return world?.editingCoasterId ?? null; },
  trainStates() {
    return {
      coaster: parseTrainStates(current?.coasters[0]?.trainStates ?? new Float32Array()),
      construction: parseTrainStates(current?.coasters.find(({ id }) => id === world?.editingCoasterId)?.trainStates ?? new Float32Array()),
    };
  },
  replaySession: replayLiveSession,
  advanceTicks(count) {
    if ((!frozen && !(testMode && gamePause.isPaused())) || advanceTicksComplete || !Number.isInteger(count) || count < 0 || count > 100000) throw new Error("Invalid test tick advance");
    return new Promise((resolve) => {
      advanceTicksComplete = resolve;
      sendWorkerAction(worker, { type: "test-advance", count });
    });
  },
  advanceBlockAnalysis(budget) {
    if (!manualBlockAnalysis || advanceBlockAnalysisComplete) throw new Error("Invalid test block analysis advance");
    validateBlockAnalysisBudget(budget);
    return new Promise<void>((resolve) => { advanceBlockAnalysisComplete = resolve; sendWorkerAction(worker, { type: "test-advance-block-analysis", budget }); });
  },
  advanceBlockPlacement(budget) {
    if (!manualBlockAnalysis || advanceBlockAnalysisComplete) throw new Error("Invalid test block placement advance");
    validateBlockAnalysisBudget(budget);
    return new Promise<void>((resolve) => { advanceBlockAnalysisComplete = resolve; sendWorkerAction(worker, { type: "test-advance-block-placement", budget }); });
  },
  setCamera(value) { camera = value; followPoints = undefined; renderRevision.invalidate(); },
  cutaway() { return renderedCutaway ?? null; },
  setSurface(x, y, corners) { sendWorkerAction(worker, { type: "set-surface", x, y, corners: new Int16Array(corners) }); },
  project(point) { return projectWorld(screenCamera(), viewport, point); },
  pick(x, y) { return renderer.pick(x, y); },
  pickRegion(x, y, width, height) { return renderer.pickRegion(x, y, width, height); },
  pickSurface(x, y) { return renderer.pickSurface(x, y); },
  setTimeOfDay,
  timeOfDay() { return dayPhase; },
  crestBursts() { return crestHistory; },
};
window.gameTest = gameTest;
const heightMarkers = document.querySelector<HTMLElement>("[data-height-markers]")!;
const heightMarkerNodes = new Map<string, HTMLElement>();
let cachedHeightWorld: CoasterWorld | undefined;
let cachedHeightRails: Float32Array | undefined;
let cachedConstructionHeights: ReturnType<typeof constructionHeights> = null;
const unitsContainer = document.querySelector<HTMLElement>("[data-units-setting]")!;
function applyUnits(units: Units): void {
  panel.setUnits(units);
  landscapingPanel.setUnits(units);
  guestPanel.setUnits(units);
  rideWindow.setUnits(units);
  testingPanel.setUnits(units);
  forceOverlay.setEstimate(construction?.constructionForcePreview ?? null, units);
  renderRevision.invalidate();
}
const unitsSetting = createUnitsSetting(unitsContainer, dispatch);
applyUnits(unitsSetting.units());
const soundContainer = document.querySelector<HTMLElement>("[data-sound-setting]")!;
const soundSetting = createSoundSetting(soundContainer, dispatch);
const audio = createLiveAudio({ testMode: frozen, onLockChange: (locked) => soundSetting.setLocked(locked) });
audio.setVolume(soundSetting.volume, soundSetting.muted);
gameCanvas.addEventListener("webglcontextlost", () => { renderRevision.invalidate(); });
gameCanvas.addEventListener("webglcontextrestored", () => { renderRevision.invalidate(); });
toolbar.setActive(activeTool);
fitCanvas(false);

function handleCoasterPointer(action: InputAction): boolean {
  if (construction && !construction.complete) return true;
  if (action.type === "select") {
    const block = blockAt(action.position);
    if (block === null) return false;
    selectBlock(block);
  } else if (action.type === "block-marker-drag-start") {
    selectBlock(blockAt(action.position));
  } else if (action.type === "block-marker-drag-end") {
    if (construction && selectedBlock !== null) {
      const distance = nearestBlockBoundaryMetres(construction.placedRails, construction.railDistances, action.position, screenCamera(), viewport);
      if (distance !== undefined) {
        const closest = Array.from(construction.blockCandidates).reduce<number | undefined>((best, candidate) => best === undefined || Math.abs(candidate - distance) < Math.abs(best - distance) ? candidate : best, undefined);
        sendWorkerAction(worker, { type: "construction-move-block", index: selectedBlock, boundaryMetres: closest !== undefined && Math.abs(closest - distance) < 1 ? closest : distance });
      }
    }
  } else if (action.type === "block-marker-nudge") {
    const section = construction?.blockReport.sections.find((item) => item.editableIndex === selectedBlock);
    if (section && selectedBlock !== null && construction) {
      const candidates = Array.from(construction.blockCandidates).filter((candidate) => action.step * (candidate - section.boundaryMetres) > 1e-7);
      const target = action.step > 0 ? candidates[0] : candidates.at(-1);
      if (target !== undefined) sendWorkerAction(worker, { type: "construction-move-block", index: selectedBlock, boundaryMetres: target });
    }
  } else if (action.type === "block-marker-remove") {
    if (selectedBlock !== null) sendWorkerAction(worker, { type: "construction-remove-block", index: selectedBlock });
  } else return false;
  return true;
}
const coasterAdapter = coasterToolAdapter((action) => {
  if (action.type === "block-marker-nudge" || action.type === "block-marker-remove") return handleCoasterPointer(action);
  if (action.type === "chain-toggle") {
    if (construction?.active) sendConstructionInput({ type: "construction-set-chain", enabled: !construction.chain });
  } else if (action.type === "construction-color-select") {
    if (construction?.colors) sendConstructionInput({ type: "construction-set-colors", colors: repaint(construction.colors, action.part, action.color) });
  } else if (action.type === "construction-open-toggle") {
    if (construction?.active) sendConstructionInput({ type: "construction-set-open", open: !construction.isOpen });
  } else if (action.type === "construction-price-step") {
    if (construction?.active) {
      const price = construction.coasterPrice + action.step;
      if (price >= 0 && price <= 65535) sendConstructionInput({ type: "construction-set-coaster-price", price });
    }
  } else if (action.type === "construction-train-count-step") {
    if (construction?.active) {
      const count = steppedTrainCount(construction.trainCount, construction.blockReport.trainCapacity, action.step);
      if (count !== undefined) sendConstructionInput({ type: "construction-set-train-count", count });
    }
  } else if (action.type === "construction-cars-per-train-step") {
    if (construction?.active && construction.trainLayout && !construction.isOpen) {
      const cars = construction.trainLayout.carsPerTrain + action.step;
      if (cars >= construction.minCarsPerTrain && cars <= construction.maxCarsPerTrain) sendConstructionInput({ type: "construction-set-cars-per-train", cars });
    }
  } else if (action.type === "construction-delete-mode" || action.type === "delete-mode-toggle") {
    deleteMode = action.type === "delete-mode-toggle" ? !deleteMode : action.enabled;
    panel.setDeleteMode(deleteMode);
    if (deleteMode && stationTool.state().active) dispatch({ type: "station-placement", enabled: false });
  } else if (action.type === "piece-step" || action.type === "piece-group") {
    panel.choose(action);
  } else if (action.type === "construction-set-model") {
    sendConstructionInput({ type: "construction-select", piece: `model:${action.model}` });
  } else if (isConstructionAction(action)) {
    sendConstructionInput(action);
  } else return false;
  return true;
}, handleCoasterPointer);

let executingConstructionInput = false;
let constructionInputSent = false;
function sendConstructionInput(action: Extract<InputAction, WorkerAction>): void {
  constructionInputSent = true;
  panel.setInputPending(true);
  sendWorkerAction(worker, action);
}
const constructionInputQueue = createConstructionInputQueue((action) => {
  constructionInputSent = false;
  executingConstructionInput = true;
  try { coasterAdapter.key(action); } finally { executingConstructionInput = false; }
  if (constructionInputSent) sendWorkerAction(worker, { type: "construction-input-barrier" });
  return constructionInputSent;
}, (notice) => panel.setQueueNotice(notice));

function dispatch(action: InputAction): void {
  inputLog.push({ action: structuredClone(action), time: performance.now(), tick: current?.tickCount ?? 0, viewport: { ...viewport }, camera: { ...camera } });
  if (action.type === "hover" || action.type === "drag-move") {
    pointerPosition = action.position;
    if (renderedCutaway) renderRevision.invalidate();
  }
  // Coaster keys only act while the coaster tool is open; its buttons are hidden otherwise.
  if (coasterActions.has(action.type) || action.type.startsWith("construction-")) {
    if (executingConstructionInput) coasterAdapter.key(action);
    else if (activeTool === "coaster") constructionInputQueue.submit(action, construction?.track.length ?? 0);
    return;
  }
  if (action.type === "session-export") {
    if (loadedPark) { parkSaves.message("Session export is unavailable after loading a park."); return; }
    const requestId = ++sessionExportRequest;
    pendingSessionExports.set(requestId, inputLog.slice());
    sendWorkerAction(worker, { type: "export-session", requestId });
  } else if (action.type === "game-speed-cycle") {
    const speed = dayClock.cycleGameSpeed();
    gameSpeed = speed;
    renderClock.setSpeed(speed);
    sendWorkerAction(worker, { type: "set-speed", speed });
  } else if (action.type === "pause-toggle") {
    const paused = gamePause.toggle();
    renderClock.setPaused(paused);
    dayClock.setPaused(paused);
    sendWorkerAction(worker, { type: "set-paused", paused });
  } else if (action.type === "tool-select") {
    openTool(action.tool);
  } else if (action.type === "day-scrub") {
    setTimeOfDay(action.phase);
  } else if (action.type === "day-speed-set") {
    daySpeed = action.speed;
    dayClock.setDaySpeed(action.speed);
  } else if (action.type === "testing-metric-set") {
    testingMetric = action.metric;
    testingPanel.setMetric(action.metric);
    renderRevision.invalidate();
  } else if (action.type === "theme-set") {
    themeToggle.setTheme(action.theme);
    renderRevision.invalidate();
  } else if (action.type === "units-set") {
    unitsSetting.setUnits(action.units);
    applyUnits(action.units);
  } else if (action.type === "sound-mute-toggle") {
    soundSetting.setMuted(!soundSetting.muted);
    audio.setVolume(soundSetting.volume, soundSetting.muted);
  } else if (action.type === "sound-volume") {
    soundSetting.setVolume(action.volume);
    audio.setVolume(soundSetting.volume, soundSetting.muted);
  } else if (action.type === "ride-select") {
    if (selectedGuestId) dispatch({ type: "guest-close" });
    selectedRide = action.ride;
    latestRideInfoJson = "";
    sendWorkerAction(worker, { type: "ride-info-request", ride: action.ride });
  } else if (action.type === "ride-test") {
    sendWorkerAction(worker, { type: "ride-test", ride: action.ride });
  } else if (action.type === "ride-edit") {
    sendWorkerAction(worker, action);
  } else if (action.type === "ride-set-open" || action.type === "ride-set-price" || action.type === "ride-rename" || action.type === "ride-demolish") {
    sendWorkerAction(worker, action);
  } else if (action.type === "ride-close") {
    selectedRide = undefined;
    latestRideInfo = null;
    latestRideInfoJson = "";
    rideWindow.close();
    sendWorkerAction(worker, { type: "ride-info-close" });
    renderRevision.invalidate();
  } else if (action.type === "guest-select") {
    if (selectedRide !== undefined) dispatch({ type: "ride-close" });
    selectedGuestId = action.guestId;
    sendWorkerAction(worker, { type: "guest-detail-request", guestId: action.guestId });
    renderRevision.invalidate();
  } else if (action.type === "guest-close") {
    selectedGuestId = undefined;
    guestPanel.close();
    sendWorkerAction(worker, { type: "guest-detail-close" });
    renderRevision.invalidate();
  } else if (action.type === "facility-place-mode") {
    openTool("facilities", false);
    facilityAdapter.activate({ mode: "place", kind: action.kind, rotation: action.rotation });
  } else if (action.type === "facility-remove-mode") {
    openTool("facilities", false);
    facilityAdapter.activate({ mode: "remove" });
  } else if (action.type === "facility-cancel") {
    facilityAdapter.deactivate();
  } else if (action.type === "facility-price") {
    sendWorkerAction(worker, { type: "facility-price", id: action.id, price: action.price });
  } else if (action.type === "path-build-selection") {
    openTool("paths", false);
    followPoints = undefined;
    pathAdapter.activate({ mode: "build", selection: action.selection });
  } else if (action.type === "path-remove-selection") {
    openTool("paths", false);
    followPoints = undefined;
    pathAdapter.activate({ mode: "remove" });
  } else if (action.type === "path-cancel") {
    pathAdapter.deactivate();
  } else if (action.type === "scenery-place-mode") {
    openTool("scenery", false);
    sceneryAdapter.activate({ mode: "place", kind: action.kind, rotation: action.rotation });
  } else if (action.type === "scenery-remove-mode") {
    openTool("scenery", false);
    sceneryAdapter.activate({ mode: "remove" });
  } else if (action.type === "scenery-cancel") {
    sceneryAdapter.deactivate();
  } else if (action.type === "landscaping-activate") {
    openTool("landscape", false);
    followPoints = undefined;
    landscapingAdapter.activate(action.selection);
  } else if (action.type === "landscaping-cancel") {
    landscapingAdapter.deactivate();
  } else if (action.type === "underground-cutaway-mode-set") {
    undergroundCutawayMode = action.mode;
    storeUndergroundCutawayMode(undergroundCutawayMode);
    panel.setUndergroundCutawayMode(undergroundCutawayMode);
    if (undergroundCutawayMode === "off") manualUndergroundEnabled = false;
    syncLandscapingUndergroundView(automaticCutawayTarget(), true);
    renderRevision.invalidate();
  } else if (action.type === "underground-view-toggle" || action.type === "underground-view-level" || action.type === "underground-view-style") {
    if (action.type === "underground-view-level") selectedUndergroundLevel = action.level;
    else if (action.type === "underground-view-style") undergroundStyle = action.style;
    else if (automaticCutawayVisible(automaticCutawayTarget())) {
      undergroundCutawayMode = "off";
      storeUndergroundCutawayMode(undergroundCutawayMode);
      panel.setUndergroundCutawayMode(undergroundCutawayMode);
      manualUndergroundEnabled = false;
    } else manualUndergroundEnabled = !manualUndergroundEnabled;
    syncLandscapingUndergroundView(automaticCutawayTarget(), true);
    renderRevision.invalidate();
  } else if (landscapingTool.state().active && (action.type === "hover" || action.type === "select" || action.type === "drag-start" || action.type === "drag-move" || action.type === "drag-end" || action.type === "drag-cancel")) {
    if (action.type === "drag-cancel") landscapingAdapter.drag("cancel", null);
    else {
      const state = landscapingTool.state();
      const carvingWithAutomaticCutaway = !manualUndergroundEnabled && state.selection?.tool === "carve" && automaticCutawayVisible(automaticCutawayTarget());
      const pick = action.position
        ? carvingWithAutomaticCutaway ? renderer.pickSurfaceUncut(action.position.x, action.position.y) : renderer.pickSurface(action.position.x, action.position.y)
        : null;
      const undergroundLevel = carvingWithAutomaticCutaway ? undefined : pickLevelAt(action.position);
      const tile = landscapingTileFromPick(pick, world, action.position, screenCamera(), viewport, undergroundLevel);
      if (action.type === "hover") landscapingAdapter.pointerMove(tile);
      else if (action.type === "select") landscapingAdapter.click(tile);
      else landscapingAdapter.drag(action.type === "drag-start" ? "start" : action.type === "drag-move" ? "move" : "end", tile);
    }
  } else if (sceneryTool.state().mode !== "inactive" && (action.type === "hover" || action.type === "select")) {
    const position = action.position;
    const surface = position ? renderer.pickSurface(position.x, position.y) : null;
    const tile = sceneryTileFromPick(surface, position, screenCamera(), viewport);
    if (action.type === "hover") sceneryAdapter.pointerMove({ tile, id: undefined });
    else {
      const selection = renderer.pick(action.position.x, action.position.y);
      sceneryAdapter.click({ tile, id: entityIdFromPick(selection, "scenery") });
    }
  } else if (action.type === "path-build-mode" || action.type === "path-remove-mode") {
    openTool("paths", false);
    pathsElement.querySelector<HTMLButtonElement>(action.type === "path-build-mode" ? "[data-build]" : "[data-remove]")!.click();
  } else if (pathTool.state().mode !== "inactive" && (action.type === "hover" || action.type === "select" || action.type === "drag-start" || action.type === "drag-move" || action.type === "drag-end" || action.type === "drag-cancel")) {
    if (action.type === "drag-cancel") pathAdapter.drag("cancel", null);
    else {
      const pick = action.position && world ? renderer.pickSurface(action.position.x, action.position.y) : null;
      const tile = world ? pathTileFromPick(pick, world) : null;
      pathHover = tile;
      if (action.type === "hover") pathAdapter.pointerMove(tile);
      else if (action.type === "select") pathAdapter.click(tile);
      else pathAdapter.drag(action.type === "drag-start" ? "start" : action.type === "drag-move" ? "move" : "end", tile);
    }
  } else if (action.type === "station-placement") {
    if (action.enabled) {
      openTool("coaster", false); deleteMode = false; panel.setDeleteMode(false);
      stationAdapter.activate(undefined);
    } else stationAdapter.deactivate();
  } else if (action.type === "station-height") {
    stationTool.setHeight(action.height);
  } else if (stationTool.state().active && (action.type === "hover" || action.type === "select")) {
    const position = action.position;
    const selection = position ? renderer.pick(position.x, position.y) : null;
    const surface = position ? renderer.pickSurface(position.x, position.y) : null;
    const tile = stationTileFromSurfacePick(surface);
    const preserve = !!selection && "id" in selection && selection.id.startsWith("preview-");
    const pointer = { position, tile, preserve, project: (point: WorldPoint) => projectWorld(screenCamera(), viewport, point) };
    if (action.type === "hover") stationAdapter.pointerMove(pointer);
    else stationAdapter.click(pointer);
  } else if (facilityTool.state().mode !== "inactive" && (action.type === "hover" || action.type === "select")) {
    const position = action.position;
    const selection = position ? renderer.pick(position.x, position.y) : null;
    const surface = position ? renderer.pickSurface(position.x, position.y) : null;
    const facility = position ? renderer.pickFacilityPlacement(position.x, position.y) : null;
    const tile = facilityTileFromPicks(surface, facility);
    const pointer = { tile, id: facilityIdFromPick(selection) };
    if (action.type === "hover") {
      if (facilityTool.state().mode === "place" && !tile) facilityPanel.previewDirection(undefined);
      facilityAdapter.pointerMove(pointer);
    } else facilityAdapter.click(pointer);
  } else if (action.type === "block-marker-drag-start" || action.type === "block-marker-drag-move" || action.type === "block-marker-drag-end" || action.type === "block-marker-drag-cancel") {
    const phase = action.type === "block-marker-drag-start" ? "start" : action.type === "block-marker-drag-move" ? "move" : action.type === "block-marker-drag-end" ? "end" : "cancel";
    coasterAdapter.drag(phase, action);
  } else if (action.type === "block-marker-nudge" || action.type === "block-marker-remove") {
    coasterAdapter.key(action);
  } else if (action.type === "hover" && activeTool === "coaster") {
    const estimateSamples = forceOverlayVisible() ? construction?.constructionForcePreview?.forceEnvelopeSamples ?? [] : [];
    const points = estimateSamples.map((sample) => projectWorld(screenCamera(), viewport, sample));
    if (forceOverlay.setHover(action.position ? nearestSampleIndex(points, action.position, FORCE_HOVER_PIXELS * pixelRatio) : null)) renderRevision.invalidate();
  } else if (action.type === "select") {
    const selection = renderer.pick(action.position.x, action.position.y);
    gameCanvas.dataset.selection = JSON.stringify(selection);
    if (coasterAdapter.click(action)) {
      // The coaster tool selected a block marker.
    } else if (selection && "id" in selection && selection.kind === "testing") {
      if (selectedBlock !== null) selectBlock(null);
      const eventMatch = /^proximity-(\d+)$/.exec(selection.id);
      const sampleMatch = /^testing-(\d+)$/.exec(selection.id);
      if (eventMatch) testingPanel.inspectProximity(Number(eventMatch[1]));
      else if (sampleMatch) testingPanel.inspect(Number(sampleMatch[1]));
    } else if (deleteMode && selection && "id" in selection) {
      const match = /^track-(\d+)-(\d+)$/.exec(selection.id);
      if (match && Number(match[1]) === world?.editingCoasterId) dispatch({ type: "construction-delete", id: Number(match[2]) });
    } else {
      const guestSelection = guestSelectionActionFromPick(selection);
      if (guestSelection) dispatch(guestSelection);
      else if ((activeTool === "none" || (activeTool === "coaster" && panelContainer!.hidden)) && rideFromPick(selection) !== null) {
        dispatch({ type: "ride-select", ride: rideFromPick(selection)! });
      }
    }
  } else if (action.type === "builder-cancel") {
    // Escape first leaves the active input mode, then closes the open tool. An open guest panel takes Escape for itself.
    if (selectedBlock !== null && activeTool === "coaster") selectBlock(null);
    else if (inputModeActive()) endInputModes();
    else if (!selectedGuestId && selectedRide === undefined) openTool("none");
  } else if (action.type === "pan" || action.type === "zoom" || action.type === "rotate") {
    moveCamera(action);
    if (action.type !== "rotate") followPoints = undefined;
    renderRevision.invalidate();
  }
}
function isConstructionAction(action: InputAction): action is Extract<InputAction, WorkerAction> {
  return action.type.startsWith("construction-");
}
bindInput(gameCanvas, dispatch, () => pathTool.state().mode === "build" || landscapingTool.state().active, (position) => blockAt(position) !== null, () => activeTool === "coaster" && selectedBlock !== null);

const worker = new Worker(new URL("./worker.ts", import.meta.url), {
  type: "module",
});
let parkRequestId = 0;
let loadedPark = false;
let snapLoadedPark = false;
interface ParkSnapshotReply { bytes?: Uint8Array; tickCount?: number }
const pendingParkRequests = new Map<number, { resolve: (reply: ParkSnapshotReply) => void; reject: (error: Error) => void }>();
const parkSavesContainer = document.querySelector<HTMLElement>("[data-park-saves]");
if (!parkSavesContainer) throw new Error("Park saves controls are missing");
function requestParkSnapshot(): Promise<{ bytes: Uint8Array; parkAgeTicks: number }> {
  const requestId = ++parkRequestId;
  return new Promise<{ bytes: Uint8Array; parkAgeTicks: number }>((resolve, reject) => {
    pendingParkRequests.set(requestId, {
      resolve(reply) {
        if (!reply.bytes || reply.tickCount === undefined) {
          reject(new Error("Worker returned no park snapshot age or data."));
          return;
        }
        resolve({ bytes: reply.bytes, parkAgeTicks: reply.tickCount });
      },
      reject,
    });
    sendWorkerAction(worker, { type: "save-park", requestId });
  });
}
async function loadParkBytes(bytes: Uint8Array): Promise<void> {
  const requestId = ++parkRequestId;
  await new Promise<void>((resolve, reject) => {
    pendingParkRequests.set(requestId, { resolve: () => resolve(), reject });
    sendWorkerAction(worker, { type: "load-park", requestId, bytes });
  });
}
const parkSaves = createParkSaves(parkSavesContainer, async () => {
  return (await requestParkSnapshot()).bytes;
}, loadParkBytes, (settings) => autosaveScheduler?.setSettings(settings));
autosaveScheduler = createAutosaveScheduler({
  settings: getAutosaveSettings(),
  capture: requestParkSnapshot,
  message(text, openMenu) {
    parkSaves.message(text, openMenu ?? true);
    void parkSaves.refreshAutosaves().catch((error: unknown) => {
      parkSaves.message(`Could not refresh autosaves: ${error instanceof Error ? error.message : String(error)}`);
    });
  },
});
document.addEventListener("visibilitychange", () => {
  if (document.hidden && autosaveScheduler) void autosaveScheduler.update(performance.now(), false, current?.tickCount ?? 0);
});
let workerReady = false;
let replayInProgress = false;
const params = new URLSearchParams(location.search);
const seedText = params.get("seed") ?? "2026";
if (!/^\d+$/.test(seedText) || BigInt(seedText) > 18446744073709551615n) throw new RangeError("Invalid seed");
const park = params.get("park") ?? "demo";
const guestCount = testAgents === null ? 12 : Number(testAgents);
if (!Number.isInteger(guestCount) || guestCount < 1 || guestCount > 500) throw new RangeError("Invalid test guest count");
if (park === "new") {
  sendWorkerAction(worker, { type: "init", seed: BigInt(seedText), park, guestCount, frozen, manualBlockAnalysis });
} else if (park === "demo") {
  const response = await fetch(new URL("./starter.park.save", document.baseURI));
  if (!response.ok) throw new Error(`Could not load generated starter park save: ${response.status}`);
  const saveBytes = new Uint8Array(await response.arrayBuffer());
  sendWorkerAction(worker, { type: "init", seed: BigInt(seedText), park: "save", saveBytes, guestCount, frozen, manualBlockAnalysis });
} else {
  const descriptionJson = bundledParkDescriptionJson(park);
  loadedPark = true;
  sendWorkerAction(worker, { type: "init", seed: BigInt(seedText), park: "description", descriptionJson, guestCount, frozen, manualBlockAnalysis });
}
worker.addEventListener("message", (event: MessageEvent<WorkerMessage>) => {
  const message = event.data;
  if (message.type === "ready") { workerReady = true;
  } else if (message.type === "initialization-error") {
    parkSaves.message(`Could not load park: ${message.error}`);
  } else if (message.type === "park-saved" || message.type === "park-loaded") {
    const pending = pendingParkRequests.get(message.requestId);
    if (!pending) throw new Error("Unknown park request");
    pendingParkRequests.delete(message.requestId);
    if (message.error) pending.reject(new Error(message.error));
    else {
      if (message.type === "park-loaded") loadedPark = true;
    pending.resolve(message.type === "park-saved" ? { bytes: message.bytes, tickCount: message.tickCount } : {});
    }
  } else if (message.type === "park-restoring") {
    autosaveScheduler?.reset();
    snapLoadedPark = true;
    dayTick = undefined;
    coasterCrests = new Map();
    crestBursts = [];
    soundMemory = undefined;
    previous = undefined;
    current = undefined;
    if (selectedGuestId) dispatch({ type: "guest-close" });
    if (selectedRide !== undefined) dispatch({ type: "ride-close" });
  } else if (message.type === "session-export") {
    const inputs = pendingSessionExports.get(message.requestId);
    if (!inputs) throw new Error("Unknown session export request");
    pendingSessionExports.delete(message.requestId);
    if (message.error) { parkSaves.message(message.error); return; }
    if (!message.recording) throw new Error("Worker returned no session recording");
    const session = { format: "theme-park-session", version: 1, build: { gitHash: __GIT_HASH__, mode: import.meta.env.MODE }, ...message.recording, inputs };
    const url = URL.createObjectURL(new Blob([JSON.stringify(session, null, 2)], { type: "application/json" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `session-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
    link.click();
    URL.revokeObjectURL(url);
  } else if (message.type === "facility-access-preview") {
    if (facilityTool.receivePreview(message.requestId, message.rotation)) facilityPanel.previewDirection(message.rotation);
  } else if (message.type === "scenery") {
    if (message.outcome === "placed" || message.outcome === "removed") autosaveScheduler?.markChanged();
    scenery = message.scenery;
    if (message.message) sceneryTool.receive(message.message);
    queueUiEvent(placementUiEvent(message.outcome));
    renderRevision.invalidate();
  } else if (message.type === "terrain-result") {
    if (message.success) autosaveScheduler?.markChanged();
    landscapingTool.receive(message);
    queueUiEvent(terrainUiEvent(message));
    landscapingPanel.message(message.message);
  } else if (message.type === "path-preview" || message.type === "path-command-result") {
    if (message.type === "path-command-result" && message.success) autosaveScheduler?.markChanged();
    pathTool.receive(message);
    if (message.type === "path-command-result") queueUiEvent(pathUiEvent(message));
  } else if (message.type === "world") {
    const coasters = message.coasters.map((update): CoasterRenderData => {
      const previous = world?.coasters.find(({ id }) => id === update.id);
      const geometry = update.geometry ?? previous?.geometry;
      const rails = update.rails ?? previous?.rails;
      const ties = update.ties ?? previous?.ties;
      const supports = update.supports ?? previous?.supports;
      if (!geometry || !rails || !ties || !supports) throw new Error(`Initial coaster geometry is missing for ${update.id}`);
      return { ...update, geometry, rails, ties, supports };
    });
    world = { ...message, coasters };
    testingPanel.setWorld(world);
    landscapingTool.setDimensions(world.dimensions[0], world.dimensions[1]);
    renderRevision.invalidate();
  } else if (message.type === "test-advanced") {
    advanceTicksComplete?.({ queueEntryPlaceIds: message.queueEntryPlaceIds });
    advanceTicksComplete = undefined;
  } else if (message.type === "test-block-analysis-advanced") {
    advanceBlockAnalysisComplete?.();
    advanceBlockAnalysisComplete = undefined;
  } else if (message.type === "facilities") {
    if (message.outcome === "placed" || message.outcome === "removed" || message.message === "Price updated.") autosaveScheduler?.markChanged();
    if (frozen && facilities.length && !message.message && !message.forced) return;
    facilities = message.facilities;
    queueUiEvent(placementUiEvent(message.outcome));
    const panelSnapshot = JSON.stringify([facilities.map(({ id, kind, price }) => ({ id, kind, price })), message.revenue]);
    if (panelSnapshot !== facilityPanelSnapshot) {
      facilityPanel.update(facilities, message.revenue);
      facilityPanelSnapshot = panelSnapshot;
    }
    if (message.placed) facilityTool.placed();
    if (message.message) facilityPanel.message(message.message);
    renderRevision.invalidate();
  } else if (message.type === "coasters") {
    coasterQueueLengths = message.coasters.map(({ id, queueLength }) => ({ id, queueLength }));
    coasterList.update(message.coasters);
  } else if (message.type === "coaster-action-result") {
    if (message.success && message.action === "edit") {
      openTool("coaster", false);
      showCoasterBuilder();
      dispatch({ type: "ride-close" });
    } else if (message.success && message.action === "demolish" && selectedRide === message.coasterId) {
      dispatch({ type: "ride-close" });
    } else if (!message.success && latestRideInfo && selectedRide === message.coasterId) {
      rideWindow.update(latestRideInfo, message.message);
    }
  } else if (message.type === "piece-shapes") {
    panel.setShapes(message.shapes);
  } else if (message.type === "construction-finish-status") {
    if (construction) {
      construction = { ...construction, canFinish: message.canFinish, finishReason: message.finishReason };
      panel.updateFinishStatus(message.canFinish, message.finishReason);
    }
  } else if (message.type === "construction-state" || message.type === "block-analysis-progress") {
    if (message.type === "block-analysis-progress" && !construction) throw new Error("Block analysis progress arrived before construction state");
    const next: ConstructionState = message.type === "construction-state" ? message : { ...construction!, ...message, type: "construction-state" };
    const redraw = message.type === "construction-state" || blockLayoutChanged(construction!.blockReport.sections, next.blockReport.sections);
    if (!frozen) constructionEffects.observe(construction, next, performance.now());
    for (const event of constructionUiEvents(construction, next)) queueUiEvent(event);
    const preview = railPoints(next.previewRails);
    const previewKey = JSON.stringify([preview[0], preview.at(-1)]);
    if (pathTool.state().mode === "inactive" && preview.length && previewKey !== followedPreview) followPoints = preview;
    followedPreview = previewKey;
    construction = next;
    panel.update(next);
    if (message.type === "construction-state") constructionInputQueue.setBusy(!next.complete, next.track.length);
    const selectedWouldTunnel = next.pieces.some((piece) => piece.id === next.selected && piece.wouldTunnel)
      || next.elements.some((element) => element.id === next.selected && element.wouldTunnel);
    tunnelPreview.textContent = selectedWouldTunnel ? "This selection will tunnel through terrain." : "";
    tunnelPreview.hidden = activeTool !== "coaster" || !selectedWouldTunnel;
    if (selectedBlock !== null && !next.blockReport.sections.some((section) => section.editableIndex === selectedBlock)) selectBlock(null);
    gameCanvas.dataset.blockBoundaries = String(next.blockReport.sections.length);
    const reportJson = JSON.stringify(next.testingReport ?? null);
    if (reportJson !== testingReportJson) { testingReportJson = reportJson; testingPanel.update(next.testingReport ?? null); }
    const estimateJson = JSON.stringify(next.constructionForcePreview ?? null);
    if (estimateJson !== constructionForcePreviewJson) { constructionForcePreviewJson = estimateJson; testingPanel.updateEstimate(next.constructionForcePreview ?? null); forceOverlay.setEstimate(next.constructionForcePreview ?? null, unitsSetting.units()); }
    if (redraw) renderRevision.invalidate();
  } else if (message.type === "ride-info") {
    if (message.info.ride !== selectedRide) return;
    const rideInfoJson = JSON.stringify([message.info, message.testMessage]);
    if (rideInfoJson === latestRideInfoJson) return;
    latestRideInfoJson = rideInfoJson;
    latestRideInfo = message.info;
    rideWindow.update(message.info, message.testMessage);
    renderRevision.invalidate();
  } else if (message.type === "guest-detail") {
    latestGuestDetail = message.detail;
    if (message.guestId === selectedGuestId && message.detail) guestPanel.update(message.detail);
  } else if (message.type === "snapshot") {
    for (const coaster of message.coasters) validateTrainStates(coaster.trainStates);
    if (frameLogEnabled) snapshotArrivalEntries.push({ now: performance.now(), sentAt: message.sentAt, tickCount: message.tickCount });
    if (snapLoadedPark) {
      renderClock.snap(message);
      snapLoadedPark = false;
      renderRevision.invalidate();
    } else if (!frozen && advanceTicksComplete) {
      renderClock.snap(message);
      renderRevision.invalidate();
    } else if (!frozen && !renderClock.push(message)) return;
    previous = current;
    if (!frozen || !current || current.guestIds.length === 0 || advanceTicksComplete) current = message;
    if (!frozen) observeTick(message);
    else if (advanceTicksComplete) observeCrests(message);
    output.value = String(message.tickCount);
    gameCanvas.dataset.agentCount = String(current?.guestIds.length ?? 0);
  } else if (message.type === "construction-input-barrier") {
    panel.setInputPending(false);
    constructionInputQueue.complete(construction?.track.length ?? 0, construction?.active ?? false);
  } else if (message.type === "surface-edited" || message.type === "test-resumed" || message.type === "construction-finish-ready") {
    if (message.type === "surface-edited" && message.changed) autosaveScheduler?.markChanged();
    // These replies are consumed by test and replay listeners.
  } else {
    assertNever(message);
  }
});

async function replayLiveSession(value: ReplaySession): Promise<void> {
  const session = parseReplaySession(value);
  if (!frozen || !manualBlockAnalysis) throw new Error("Session replay requires frozen test mode with manual block analysis");
  if (!workerReady || gameTest.tickCount() !== 0) throw new Error("Session replay requires a ready worker at tick zero");
  if (BigInt(session.seed) !== BigInt(seedText) || session.park !== park || session.guestCount !== guestCount) {
    throw new Error("Session replay settings must match the initialized worker");
  }
  if (replayInProgress) throw new Error("A session replay is already in progress");
  replayInProgress = true;
  try {
    await replaySessionIntoWorker(worker, session, { action: { type: "test-resume" }, replyType: "test-resumed" });
    frozen = false;
    manualBlockAnalysis = false;
    dayTick = undefined;
    renderRevision.invalidate();
  } finally {
    replayInProgress = false;
  }
}

function observeTick(snapshot: CoasterSnapshot): void {
  if (dayTick !== undefined) dayPhase = advanceDayPhase(dayPhase, Math.max(0, snapshot.tickCount - dayTick), daySpeed);
  dayTick = snapshot.tickCount;
  observeCrests(snapshot);
}

function observeCrests(snapshot: CoasterSnapshot): void {
  const newBursts: CrestBurst[] = [];
  const nextCrests = new Map<number, ReturnType<typeof createCrestTracker>>();
  for (const coaster of snapshot.coasters) {
    const result = detectCrest(coasterCrests.get(coaster.id) ?? createCrestTracker(), coaster.trainPoses, snapshot.tickCount);
    nextCrests.set(coaster.id, result.tracker);
    if (result.burst) newBursts.push({ ...result.burst, coasterId: coaster.id });
  }
  coasterCrests = nextCrests;
  crestHistory.push(...newBursts);
  crestBursts = [...crestBursts, ...newBursts].filter((burst) => snapshot.tickCount - burst.tick < CONFETTI_LIFE_TICKS);
}

/** The cutaway window follows the end of the track being built, cut just above it; otherwise it follows the pointer at the chosen cut level. */
function cutawayFor(level: number, trackEnd: WorldPoint | undefined): Cutaway {
  const center = trackEnd ? projectWorld(screenCamera(), viewport, trackEnd) : pointerPosition ?? { x: viewport.width / 2, y: viewport.height / 2 };
  return { x: center.x, y: center.y, radius: CUTAWAY_RADIUS * viewport.width / cssViewport().width, level: trackEnd ? cutawayLevelForTrack(trackEnd.z) : level };
}

function automaticCutawayTarget(heights?: ReturnType<typeof constructionHeights>): UndergroundCutawayTarget | null {
  if (!world) return null;
  if (activeTool === "coaster" && construction?.active) {
    const constructionHeightsForTrack = heights ?? currentConstructionHeights();
    return constructionCutawayTarget(constructionHeightsForTrack?.end ?? null);
  }
  if (activeTool === "paths" && pathTool.state().mode === "build") {
    const preview = pathTool.state().preview;
    return pathCutawayTarget(world, preview?.paths ?? null) ?? pathTileCutawayTarget(world, pathHover);
  }
  if (activeTool === "landscape" && landscapingTool.state().active) {
    const state = landscapingTool.state();
    return landscapingCutawayTarget(world, state.preview, state.selection);
  }
  return null;
}

function currentConstructionHeights(): ReturnType<typeof constructionHeights> {
  if (!world || !construction?.active) return null;
  if (cachedHeightWorld !== world || cachedHeightRails !== construction.placedRails) {
    cachedHeightWorld = world;
    cachedHeightRails = construction.placedRails;
    cachedConstructionHeights = constructionHeights(world, construction.placedRails);
  }
  return cachedConstructionHeights;
}

function automaticCutawayVisible(target: UndergroundCutawayTarget | null): boolean {
  return shouldShowAutomaticCutaway(undergroundCutawayMode, target);
}

function syncLandscapingUndergroundView(target = automaticCutawayTarget(), force = false): void {
  const enabled = manualUndergroundEnabled || automaticCutawayVisible(target);
  if (!force && enabled === lastLandscapingUndergroundEnabled) return;
  lastLandscapingUndergroundEnabled = enabled;
  landscapingPanel.setUndergroundView(enabled, selectedUndergroundLevel, undergroundStyle);
}

/** The height the pointer picks at: the cut level inside the cutaway window or under the whole-view cut, the surface otherwise. */
function pickLevelAt(position: ScreenPoint | null): number | undefined {
  if (renderedCutaway) return position && insideCutaway(renderedCutaway, position.x, position.y) ? renderedCutaway.level : undefined;
  return manualUndergroundEnabled && undergroundStyle === "cut" ? selectedUndergroundLevel : undefined;
}

/** Force labels and the hover readout belong to the estimate of an unfinished coaster, not to test-run reports. */
function forceOverlayVisible(): boolean {
  return activeTool === "coaster" && !!construction?.active && !construction.testingReport && !!construction.constructionForcePreview && testingMetric !== null;
}

function updateHeightMarkers(heights: NonNullable<ReturnType<typeof constructionHeights>>): ScreenBox[] {
  const units = unitsSetting.units();
  const trackText = `Track end: ${formatLength(heights.end.metres, units)} above ground`;
  const stationText = `Station: ${formatLength(heights.station.metres, units)} above ground`;
  panel.setHeightMarkers(trackText, stationText);
  const descriptors: { key: string; marker: HeightMarker; text: string; attribute: string; stationText?: string }[] = [
    { key: "end", marker: heights.end, text: trackText, attribute: "data-track-height", stationText },
    ...heights.extrema.map(({ marker, kind }: HeightExtremum, index) => ({
      key: `extremum-${index}`,
      marker,
      text: `${kind === "crest" ? "Crest" : "Valley"}: ${formatLength(marker.metres, units)} above ground`,
      attribute: kind === "crest" ? "data-crest-height" : "data-valley-height",
    })),
  ];
  const activeKeys = new Set(descriptors.map(({ key }) => key));
  for (const [key, node] of heightMarkerNodes) {
    if (!activeKeys.has(key)) {
      node.remove();
      heightMarkerNodes.delete(key);
    }
  }

  const viewport = cssViewport();
  const measured = descriptors.map((descriptor) => {
    let node = heightMarkerNodes.get(descriptor.key);
    if (!node) {
      node = document.createElement("div");
      node.className = "height-markers";
      if (descriptor.stationText !== undefined) {
        node.setAttribute("data-track-height", "");
        const trackLabel = document.createElement("strong");
        const stationLabel = document.createElement("span");
        stationLabel.setAttribute("data-station-height", "");
        node.append(trackLabel, stationLabel);
      }
      heightMarkerNodes.set(descriptor.key, node);
      heightMarkers.append(node);
    }
    if (descriptor.stationText !== undefined) {
      const trackLabel = node.querySelector<HTMLElement>("strong")!;
      const stationLabel = node.querySelector<HTMLElement>("[data-station-height]")!;
      if (trackLabel.textContent !== descriptor.text) trackLabel.textContent = descriptor.text;
      if (stationLabel.textContent !== descriptor.stationText) stationLabel.textContent = descriptor.stationText;
    } else if (node.textContent !== descriptor.text) {
      node.textContent = descriptor.text;
    }
    node.hidden = false;
    const anchor = projectWorld(camera, viewport, descriptor.marker.position);
    return { descriptor, node, anchor, width: node.offsetWidth, height: node.offsetHeight };
  });
  const occupied: ScreenBox[] = [];
  for (const { descriptor, node, anchor, width, height } of measured) {
    if (descriptor.stationText !== undefined) {
      node.dataset.heightKind = "end";
    } else {
      for (const attribute of ["data-track-height", "data-station-height", "data-crest-height", "data-valley-height"]) {
        if (attribute !== descriptor.attribute) node.removeAttribute(attribute);
      }
      if (!node.hasAttribute(descriptor.attribute)) node.setAttribute(descriptor.attribute, "");
      const heightKind = descriptor.attribute === "data-crest-height" ? "crest" : "valley";
      if (node.dataset.heightKind !== heightKind) node.dataset.heightKind = heightKind;
    }

    const markerPadding = 14;
    const placements = [
      { side: "top-right", left: anchor.x + 12, top: anchor.y - height - 12 },
      { side: "top-left", left: anchor.x - width - 12, top: anchor.y - height - 12 },
      { side: "bottom-right", left: anchor.x + 12, top: anchor.y + 12 },
      { side: "bottom-left", left: anchor.x - width - 12, top: anchor.y + 12 },
    ];
    const placement = placements.find(({ left, top }) => {
      const candidate = { left: left - markerPadding, top: top - markerPadding, width: width + markerPadding * 2, height: height + markerPadding * 2 };
      const inViewport = candidate.left >= 0 && candidate.top >= 0 && candidate.left + candidate.width <= viewport.width && candidate.top + candidate.height <= viewport.height;
      const clear = occupied.every((box) => candidate.left >= box.left + box.width + 4
        || candidate.left + candidate.width + 4 <= box.left
        || candidate.top >= box.top + box.height + 4
        || candidate.top + candidate.height + 4 <= box.top);
      return inViewport && clear;
    });
    node.hidden = placement === undefined;
    if (!placement) continue;
    node.dataset.placement = placement.side;
    node.style.left = `${placement.left}px`;
    node.style.top = `${placement.top}px`;
    occupied.push({ left: placement.left - markerPadding, top: placement.top - markerPadding, width: width + markerPadding * 2, height: height + markerPadding * 2 });
  }
  return occupied;
}

function frame(now: number): void {
  if (autosaveScheduler) {
    const running = workerReady && !frozen && !gamePause.isPaused() && gameSpeed > 0 && !document.hidden && !replayInProgress;
    void autosaveScheduler.update(now, running, current?.tickCount ?? 0);
  }
  if (!world || replayInProgress) return;
  if ((frozen || ((testMode || livePauseRender) && gamePause.isPaused())) && shouldSkipRenderFrame({
    revisionReady: gameTest.ready,
    frozen,
    testMode,
    livePauseRender,
    paused: gamePause.isPaused(),
    following: followPoints !== undefined,
  })) return;
  const renderFrame = frozen ? undefined : renderClock.frame(now);
  const renderPrevious = frozen ? previous : renderFrame?.previous;
  const renderCurrent = frozen ? current : renderFrame?.current;
  const interpolation = renderFrame?.interpolation ?? 1;
  if (followPoints) {
    const followed = followTrackEnd(camera, cssViewport(), followPoints, frozen ? 1 : 1 - Math.exp(-(now - lastFrameAt) / 140));
    camera = followed.camera;
    if (followed.settled) followPoints = undefined;
  }
  lastFrameAt = now;
  const heights = activeTool === "coaster" && construction?.active ? currentConstructionHeights() : null;
  const autoTarget = automaticCutawayTarget(heights);
  const autoCutawayVisible = automaticCutawayVisible(autoTarget);
  syncLandscapingUndergroundView(autoTarget);
  heightMarkers.hidden = heights === null;
  const heightMarkerBox = heights ? updateHeightMarkers(heights) : [];
  forceOverlay.render(forceOverlayVisible(), (sample) => projectWorld(camera, cssViewport(), sample), cssViewport(), heightMarkerBox);
  constructionEffects.active(now);
  dayClock.update(dayPhase);
  renderedCutaway = manualUndergroundEnabled && undergroundStyle === "cutaway"
    ? cutawayFor(selectedUndergroundLevel, heights?.end.position)
    : !manualUndergroundEnabled && autoCutawayVisible && autoTarget
      ? cutawayFor(autoTarget.level, autoTarget)
      : undefined;
  const scene = composeScene({
    world,
    previous: renderPrevious,
    current: renderCurrent,
    interpolation,
    camera,
    screenCamera: screenCamera(),
    viewport,
    atlas,
    facilities,
    scenery,
    undergroundLevel: manualUndergroundEnabled && undergroundStyle === "cut" ? selectedUndergroundLevel : undefined,
    construction,
    activeTool,
    selectedGuestId,
    sceneryPreview: sceneryTool.state().preview,
    landscapingPreview: landscapingTool.state().preview,
    pathPreview: pathTool.state().preview,
    stationGhost: () => stationTool.ghost(),
    testingMetric,
    testingHighlightDistance: testingPanel.selectedScoreLocation()?.distance,
    rideHighlight: rideWindow.highlight(),
    constructionEffects,
    staticTrack,
    builtTrack,
    now,
    frozen,
    dayPhase,
    theme: themeToggle.theme(),
    crestBursts,
    renderTick: renderFrame?.tick,
    setGuestAnchor: (point) => guestPanel.setAnchor(point),
  });
  const sound = soundFrame({ snapshot: renderCurrent, interpolation, camera, viewport: cssViewport(), dayPhase, gameSpeed, coasters: world.coasters, crestBursts, ui: uiEvents.drain(), paused: gamePause.isPaused(), timeSeconds: now / 1000 }, soundMemory);
  soundMemory = sound.memory;
  audio.apply(sound.frame);
  renderedGuests = scene.guests;
  renderedRiders = scene.riders;
  renderer.render({ world: scene.view, camera: scene.camera, waterFrame: scene.waterFrame, geometryPasses: scene.geometryPasses, atmospherePass: scene.atmospherePass, showBuildGrid: activeTool !== "none", cutaway: renderedCutaway });
  const trainPoses = scene.view.trainPoses;
  if (frameLogEnabled && renderCurrent && trainPoses.length >= 11) {
    frameLogEntries.push({
      now,
      tickCount: renderCurrent.tickCount,
      renderTick: renderFrame?.tick ?? renderCurrent.tickCount,
      interpolation,
      trainX: trainPoses[1],
      trainY: trainPoses[2],
      trainZ: trainPoses[3],
    });
  }
  gameCanvas.dataset.drawnAgents = String(scene.guests.length);
  renderRevision.rendered(renderer.isContextAvailable() && (current?.guestIds.length ?? 0) > 0);
}


if (timing) {
  const channel = new MessageChannel();
  const total: number[] = [];
  const cpu: number[] = [];
  let count = 0;
  let started = 0;
  channel.port1.onmessage = () => {
    if (!world) { channel.port2.postMessage(null); return; }
    const begin = performance.now();
    frame(begin);
    const cpuTime = performance.now() - begin;
    renderer.waitForGpu();
    const end = performance.now();
    count++;
    if (count === 60) started = end;
    if (count > 60) {
      total.push(end - begin);
      cpu.push(cpuTime);
      if (total.length >= 300 || end - started >= 10000) {
        const sorted = [...total].sort((a, b) => a - b);
        window.workTiming = { meanMs: total.reduce((a, b) => a + b, 0) / total.length, p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1], cpuMeanMs: cpu.reduce((a, b) => a + b, 0) / cpu.length, frames: total.length, renderer: gameCanvas.getContext("webgl2")!.getParameter(gameCanvas.getContext("webgl2")!.RENDERER) as string };
        channel.port1.close(); channel.port2.close(); return;
      }
    }
    channel.port2.postMessage(null);
  };
  channel.port2.postMessage(null);
} else {
  function animate(now: number): void { frame(now); requestAnimationFrame(animate); }
  requestAnimationFrame(animate);
}
