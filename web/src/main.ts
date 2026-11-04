import { sendWorkerAction, assertNever, type WorkerAction, type WorkerMessage, type CoasterSnapshot } from "./protocol";
import "./print-modern.css";
import "./app.css";
import { buildFacilityGeometry, facilitySprites, type Facility, type FacilityKind } from "./facilities";
import { createFacilityPanel } from "./facility-panel";
import { applyCameraAction, projectWorld, type Camera, type Viewport, type WorldPoint } from "./camera";
import { bindInput, bindKeyboard, builderShortcuts, keyHint } from "./input/bindings";
import type { CameraAction, InputAction } from "./input/actions";
import { constructionRiderPositions, validateBlockAnalysisBudget, type FrameLogEntry, type GameTest, type SnapshotArrivalEntry } from "./game-test";
import { blockLayoutChanged } from "./block-analysis-progress";
import { createWorldRenderer, loadAtlas, type AtmospherePass, type WorldView, type WorldSprite } from "./render/index";
import { advanceDayPhase, BACKDROPS, DAY_LENGTH_TICKS, lightingAt, NOON, printOnBackdrop, START_PHASE, sunAt } from "./render/daylight";
import { buildConfetti, buildFireworks, buildLights, buildShadows, buildSky, CONFETTI_LIFE_TICKS, createCrestTracker, detectCrest, parkBounds, type CrestBurst } from "./render/effects";
import { createDayClock, type DaySpeed } from "./day-clock";
import { buildPeople, riderAppearances } from "./render/people";
import { buildTrackQuads, interpolateTrainPoses, trackPaint, trackStyle } from "./render/track/geometry";
import { buildRiderQuads, buildTrainQuads } from "./render/track/trains";
import type { TrainLayout } from "./train-layout";
import type { CoasterColorScheme } from "./coaster-colors";
import { createStaticTrackCache } from "./render/track/cache";
import { RenderClock } from "./render-clock";
import { createConstructionPanel } from "./construction";
import type { BlockAnalysisProgress, ConstructionState, PieceShapesMessage } from "./construction-protocol";
import { followTrackEnd, railPoints } from "./construction-camera";
import { createConstructionEffects } from "./construction-effects";
import { buildGhostQuads } from "./render/track/ghost";
import { buildBlockBoundaryQuads, nearestBlockBoundaryMetres } from "./render/track/blocks";
import { createGuestPanel } from "./guest-panel/guest-panel";
import type { GuestDetail } from "./guest-panel/protocol";
import { buildSelectionRing } from "./guest-panel/selection-ring";
import { createPathTool, type PathToolMessage } from "./path-tool";
import { createPathPanel } from "./path-panel";
import { pathTileFromPick } from "./path-picking";
import { buildPathGhostQuads, decodePathPreview, pathPreviewRefusalAt } from "./render/path-ghost";
import { buildProximityMarkers, buildTestingRibbon, createTestingPanel, type TestingMetric } from "./coaster-testing";
import { buildLandscapingGhostQuads, createLandscapingTool, type TerrainEditResult } from "./landscaping-tool";
import { createLandscapingPanel } from "./landscaping-panel";
import { buildSceneryGhostQuads, createSceneryTool, scenerySprites, type Scenery } from "./scenery-tool";
import { createSceneryPanel } from "./scenery-panel";
import { createToolbar, type Tool } from "./toolbar";
import { createThemeToggle } from "./theme";
import type { WorkerRecording } from "./session-recording";
import { parseReplaySession, type ReplaySession } from "./session-replay";
import { replaySessionIntoWorker } from "./session-replay-worker";
import { constructionHeights } from "./height-markers";
import { formatLength, type Units } from "./units";
import { createUnitsSetting } from "./units-setting";

declare const __GIT_HASH__: string;

interface RecordedInput { action: InputAction; time: number; tick: number; viewport: Viewport; camera: Camera }
const inputLog: RecordedInput[] = [];
let sessionExportRequest = 0;
const pendingSessionExports = new Map<number, RecordedInput[]>();

const themeToggleContainer = document.querySelector<HTMLElement>("[data-theme-toggle]");
if (!themeToggleContainer) throw new Error("Theme toggle is missing");
// Created before the atlas loads, so a stored choice applies as early as possible.
const themeToggle = createThemeToggle(themeToggleContainer, () => { gameTest.ready = false; });
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
/** Demo coaster geometry, paint, and train, when present. */
type CoasterWorld = WorldView & { pathMetadata: Int32Array; coasterGeometry: Float32Array; coasterRails: Float32Array; coasterTies: Float32Array; coasterSupports: Float32Array; coasterColors: CoasterColorScheme | null; coasterTrain: TrainLayout | null };
let world: CoasterWorld | undefined;
let previous: CoasterSnapshot | undefined;
let current: CoasterSnapshot | undefined;
const renderClock = new RenderClock<CoasterSnapshot>(30);
const noGeometry = new Float32Array();
let frozen = new URLSearchParams(location.search).has("test");
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
  const match = pick && "id" in pick ? /^block-boundary-(\d+)$/.exec(pick.id) : null;
  return match ? Number(match[1]) : null;
}
function selectBlock(index: number | null): void {
  selectedBlock = index;
  panel.setSelectedBlock(index);
  gameTest.ready = false;
}
let deleteMode = false;
let stationPlacement = false;
let stationHeight = 12;
let station: { x: number; y: number; z: number; heading: number; locked: boolean } | undefined;
const stationDirections = [[1, 0], [0, 1], [-1, 0], [0, -1]] as const;
function updateStationPreview(position: { x: number; y: number }): void {
  if (station?.locked) {
    const origin = projectWorld(screenCamera(), viewport, { x: station.x + 0.5, y: station.y + 0.5, z: (station.z - stationHeight) / 4 });
    const delta = { x: position.x - origin.x, y: position.y - origin.y };
    if (Math.hypot(delta.x, delta.y) > 4) {
      let best = -Infinity;
      stationDirections.forEach(([dx, dy], index) => {
        const end = projectWorld(screenCamera(), viewport, { x: station!.x + 0.5 + dx, y: station!.y + 0.5 + dy, z: (station!.z - stationHeight) / 4 });
        const vx = end.x - origin.x, vy = end.y - origin.y;
        const score = (delta.x * vx + delta.y * vy) / Math.hypot(vx, vy);
        if (score > best) { best = score; station!.heading = index; }
      });
    }
  } else {
    const selection = renderer.pick(position.x, position.y);
    if (selection?.kind === "terrain" || selection?.kind === "carved-floor") station = { x: selection.x, y: selection.y, z: Math.ceil(selection.height / 2) * 2 + stationHeight, heading: 0, locked: false };
    else if (!selection || !("id" in selection) || !selection.id.startsWith("preview-")) station = undefined;
  }
  gameCanvas.dataset.stationPreview = station ? `${station.x},${station.y},${station.z},${station.heading}` : "";
  gameTest.ready = false;
}
function stationGhost(): Float32Array {
  if (!stationPlacement || !station) return noGeometry;
  const [dx, dy] = stationDirections[station.heading];
  const side = [-dy, dx, 0];
  const z = station.z / 4;
  return new Float32Array([-1, station.x - side[0] * 0.18, station.y - side[1] * 0.18, z, ...side, 0, 0, 1, station.x + dx * 4 - side[0] * 0.18, station.y + dy * 4 - side[1] * 0.18, z, ...side, 0, 0, 1,
    -1, station.x + side[0] * 0.18, station.y + side[1] * 0.18, z, ...side, 0, 0, 1, station.x + dx * 4 + side[0] * 0.18, station.y + dy * 4 + side[1] * 0.18, z, ...side, 0, 0, 1]);
}
const panelContainer = document.querySelector<HTMLElement>("[data-construction]");
if (!panelContainer) throw new Error("Construction panel is missing");
const panel = createConstructionPanel(panelContainer, dispatch);
// The whole page, so builder keys keep working after a click in the panel or when a re-rendered control drops focus.
bindKeyboard(document.body, dispatch, () => ({ x: gameCanvas.width / 2, y: gameCanvas.height / 2 }), () => activeTool === "coaster" && selectedBlock !== null);
const constructionEffects = createConstructionEffects(trackStyle(atlas));
let followPoints: WorldPoint[] | undefined;
let followedPreview = "";
let lastFrameAt = performance.now();
let testingMetric: TestingMetric | null = null;
let testingReportJson = "";
const testingPanel = createTestingPanel(panelContainer.querySelector<HTMLElement>("[data-testing]")!, trackStyle(atlas), (metric) => { testingMetric = metric; gameTest.ready = false; });
if (!atlas.themeStyle) throw new Error("Atlas theme style is missing");
let selectedGuestId: string | undefined;
const guestPanel = createGuestPanel(gameCanvas, {
  onSelectGuestId(id) {
    selectedGuestId = id;
    sendWorkerAction(worker, { type: "guest-detail-request", guestId: id });
    gameTest.ready = false;
  },
  onClose() {
    selectedGuestId = undefined;
    sendWorkerAction(worker, { type: "guest-detail-close" });
    gameTest.ready = false;
  },
});

let facilities: Facility[] = [];
let facilityPanelSnapshot = "";
let facilityPlacement: { kind: FacilityKind; rotation: number } | undefined;
let facilityPreviewRequestId = 0;
let facilityRemoval = false;
let advanceTicksComplete: (() => void) | undefined;
let advanceBlockAnalysisComplete: (() => void) | undefined;
const facilityContainer = document.querySelector<HTMLElement>("[data-facilities]");
if (!facilityContainer) throw new Error("Facility panel is missing");
const facilityPanel = createFacilityPanel(facilityContainer, { atlas, images }, (kind, rotation) => {
  openTool("facilities");
  facilityPreviewRequestId += 1;
  facilityPlacement = { kind, rotation };
  facilityRemoval = false;
}, () => { openTool("facilities"); facilityRemoval = true; facilityPlacement = undefined; },
() => { facilityPlacement = undefined; facilityRemoval = false; },
(id, price) => sendWorkerAction(worker, { type: "facility-price", id, price }));
const pathContainer = document.querySelector<HTMLElement>("[data-paths]");
if (!pathContainer) throw new Error("Path panel is missing");
const pathsElement = pathContainer;
let pathHover: { x: number; y: number } | null = null;
const pathTool = createPathTool((command) => sendWorkerAction(worker, command), () => {
  const state = pathTool.state();
  const refusal = state.preview && pathHover ? pathPreviewRefusalAt(decodePathPreview(state.preview.paths), pathHover.x, pathHover.y) : null;
  pathPanel.message(refusal ?? state.message);
  gameTest.ready = false;
});
const pathPanel = createPathPanel(pathContainer, (selection) => {
  openTool("paths");
  followPoints = undefined;
  pathTool.build(selection);
}, () => {
  openTool("paths");
  followPoints = undefined;
  pathTool.remove();
}, () => pathTool.cancel());
pathContainer.querySelector<HTMLElement>("[data-build]")!.title = `Build paths (${keyHint(builderShortcuts.pathBuild)})`;
pathContainer.querySelector<HTMLElement>("[data-remove]")!.title = `Remove paths (${keyHint(builderShortcuts.pathRemove)})`;
let undergroundLevel: number | undefined;
let selectedUndergroundLevel = 8;
const landscapingContainer = document.querySelector<HTMLElement>("[data-landscaping]");
if (!landscapingContainer) throw new Error("Landscaping panel is missing");
const landscapingPanel = createLandscapingPanel(landscapingContainer, dispatch);
const landscapingTool = createLandscapingTool((command) => {
  if (!world || command.x < 0 || command.y < 0 || command.x + command.size > world.dimensions[0] || command.y + command.size > world.dimensions[1]) {
    landscapingPanel.message("Brush extends outside the park.");
    return;
  }
  sendWorkerAction(worker, command);
}, () => { gameTest.ready = false; });

let scenery: Scenery[] = [];
const sceneryContainer = document.querySelector<HTMLElement>("[data-scenery]");
if (!sceneryContainer) throw new Error("Scenery panel is missing");
const sceneryTool = createSceneryTool((command) => sendWorkerAction(worker, command), () => {
  sceneryPanel.message(sceneryTool.state().message);
  gameTest.ready = false;
});
const sceneryPanel = createSceneryPanel(sceneryContainer, { atlas, images }, (kind, rotation) => {
  openTool("scenery"); sceneryTool.place(kind, rotation);
}, () => { openTool("scenery"); sceneryTool.remove(); }, () => sceneryTool.cancel());

const dock = document.querySelector<HTMLElement>("[data-dock]");
const toolbarContainer = document.querySelector<HTMLElement>("[data-toolbar]");
if (!dock || !toolbarContainer) throw new Error("Tool dock or toolbar is missing");
const toolDock = dock;
const toolPanels: Record<Exclude<Tool, "none">, HTMLElement> = { coaster: panelContainer, paths: pathContainer, landscape: landscapingContainer, facilities: facilityContainer, scenery: sceneryContainer };
const toolbar = createToolbar(toolbarContainer, (tool) => dispatch({ type: "tool-select", tool }));
let activeTool: Tool = "none";
/** The time of day follows simulation ticks, so it pauses with the game. Test mode holds it at noon unless a test sets it. */
let dayPhase = frozen ? NOON : START_PHASE;
let daySpeed: DaySpeed = 1;
let dayTick: number | undefined;
let demoCrests = createCrestTracker();
let constructionCrests = createCrestTracker();
let crestBursts: CrestBurst[] = [];
/** Every crest burst since the page loaded, for tests. */
const crestHistory: CrestBurst[] = [];
function setTimeOfDay(phase: number): void {
  dayPhase = advanceDayPhase(phase, 0, 1);
  gameTest.ready = false;
}
const dayClockContainer = document.querySelector<HTMLElement>("[data-day-clock]");
if (!dayClockContainer) throw new Error("Day clock is missing");
const dayClock = createDayClock(dayClockContainer, setTimeOfDay, (speed) => { daySpeed = speed; }, (speed) => {
  renderClock.setSpeed(speed);
  sendWorkerAction(worker, { type: "set-speed", speed });
}, () => dispatch({ type: "session-export" }));
/** Size the canvas backing store to its displayed size in device pixels, so the park is drawn sharp at any pixel ratio. */
function fitCanvas(keepCorner = true): void {
  const bounds = gameCanvas.getBoundingClientRect();
  const width = Math.round(bounds.width * devicePixelRatio);
  const height = Math.round(bounds.height * devicePixelRatio);
  if (width === gameCanvas.width && height === gameCanvas.height) return;
  const previous = cssViewport();
  gameCanvas.width = width;
  gameCanvas.height = height;
  viewport = { width, height };
  pixelRatio = width / bounds.width;
  // Keep the top-left corner of the park still, so opening a tool covers the right of the view instead of sliding it.
  if (keepCorner) camera = applyCameraAction(camera, cssViewport(), { type: "pan", dx: (previous.width - bounds.width) / 2, dy: (previous.height - bounds.height) / 2 });
  gameTest.ready = false;
}
function watchPixelRatio(): void {
  matchMedia(`(resolution: ${devicePixelRatio}dppx)`).addEventListener("change", () => { fitCanvas(); watchPixelRatio(); }, { once: true });
}
new ResizeObserver(() => fitCanvas()).observe(gameCanvas);
watchPixelRatio();
function inputModeActive(): boolean {
  return sceneryTool.state().mode !== "inactive" || stationPlacement || deleteMode || facilityPlacement !== undefined || facilityRemoval || pathTool.state().mode !== "inactive" || landscapingTool.state().active;
}
function endInputModes(): void {
  if (sceneryTool.state().mode !== "inactive") sceneryTool.cancel();
  if (landscapingTool.state().active) landscapingTool.cancel();
  if (pathTool.state().mode !== "inactive") pathTool.cancel();
  facilityPreviewRequestId += 1;
  facilityPlacement = undefined; facilityRemoval = false;
  facilityPanel.previewDirection(undefined);
  deleteMode = false; panel.setDeleteMode(false);
  if (stationPlacement) dispatch({ type: "station-placement", enabled: false });
}
/** Every input mode belongs to one tool, so opening another tool ends it and only the open tool's mode can be active. */
function openTool(tool: Tool): void {
  if (tool === activeTool) return;
  endInputModes();
  activeTool = tool;
  toolbar.setActive(tool);
  dayClock.setControlsVisible(tool === "none");
  toolDock.hidden = tool === "none";
  for (const [owner, element] of Object.entries(toolPanels)) element.hidden = owner !== tool;
  toolDock.scrollTop = 0;
  // Resize now rather than on the next observer callback, so the world is projected onto the new canvas size at once.
  fitCanvas();
  if (tool === "paths") pathPanel.activate();
  else if (tool === "landscape") landscapingPanel.activate();
  else if (tool === "scenery") sceneryPanel.activate();
  else if (tool === "facilities") facilityPanel.activate();
}
let latestGuestDetail: GuestDetail | null = null;
let renderedGuests: readonly WorldSprite[] = [];
let renderedRiders: { id: string; left: number; right: number; top: number; bottom: number }[] = [];
const gameTest: GameTest = {
  ready: false,
  tickCount() { return current?.tickCount ?? 0; },
  frameLog() { return frameLogEnabled ? frameLogEntries.map((entry) => ({ ...entry })) : []; },
  snapshotArrivals() { return frameLogEnabled ? snapshotArrivalEntries.map((entry) => ({ ...entry })) : []; },
  guests() { return renderedGuests; },
  renderedRiders() { return renderedRiders; },
  riders() {
    return current ? constructionRiderPositions(current.constructionTrainPoses, current.constructionRiders) : [];
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
  testingReport() { return construction?.testingReport ?? null; },
  constructionTrainPoses() { return Array.from(current?.constructionTrainPoses ?? []); },
  replaySession: replayLiveSession,
  advanceTicks(count) {
    if (!frozen || advanceTicksComplete || !Number.isInteger(count) || count < 1 || count > 100000) throw new Error("Invalid test tick advance");
    return new Promise<void>((resolve) => { advanceTicksComplete = resolve; sendWorkerAction(worker, { type: "test-advance", count }); });
  },
  advanceBlockAnalysis(budget) {
    if (!manualBlockAnalysis || advanceBlockAnalysisComplete) throw new Error("Invalid test block analysis advance");
    validateBlockAnalysisBudget(budget);
    return new Promise<void>((resolve) => { advanceBlockAnalysisComplete = resolve; sendWorkerAction(worker, { type: "test-advance-block-analysis", budget }); });
  },
  setCamera(value) { camera = value; followPoints = undefined; gameTest.ready = false; },
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
const trackHeightMarker = heightMarkers.querySelector<HTMLElement>("[data-track-height]")!;
const stationHeightMarker = heightMarkers.querySelector<HTMLElement>("[data-station-height]")!;
const unitsContainer = document.querySelector<HTMLElement>("[data-units-setting]")!;
function applyUnits(units: Units): void {
  panel.setUnits(units);
  landscapingPanel.setUnits(units);
  guestPanel.setUnits(units);
  testingPanel.setUnits(units);
  gameTest.ready = false;
}
const unitsSetting = createUnitsSetting(unitsContainer, applyUnits);
applyUnits(unitsSetting.units());
gameCanvas.addEventListener("webglcontextlost", () => { gameTest.ready = false; });
gameCanvas.addEventListener("webglcontextrestored", () => { gameTest.ready = false; });
toolbar.setActive(activeTool);
fitCanvas(false);

const coasterActions = new Set<InputAction["type"]>(["piece-step", "piece-group", "delete-mode-toggle", "chain-toggle"]);

function dispatch(action: InputAction): void {
  inputLog.push({ action: structuredClone(action), time: performance.now(), tick: current?.tickCount ?? 0, viewport: { ...viewport }, camera: { ...camera } });
  // Coaster keys only act while the coaster tool is open; its buttons are hidden otherwise.
  if (activeTool !== "coaster" && (coasterActions.has(action.type) || action.type.startsWith("construction-"))) return;
  if (action.type === "session-export") {
    const requestId = ++sessionExportRequest;
    pendingSessionExports.set(requestId, inputLog.slice());
    sendWorkerAction(worker, { type: "export-session", requestId });
  } else if (action.type === "game-speed-cycle") {
    dayClock.cycleGameSpeed();
  } else if (action.type === "tool-select") {
    openTool(action.tool);
  } else if (action.type === "landscaping-activate") {
    openTool("landscape");
    followPoints = undefined;
    landscapingTool.activate(action.selection);
  } else if (action.type === "landscaping-cancel") {
    landscapingTool.cancel();
  } else if (action.type === "underground-view-toggle" || action.type === "underground-view-level") {
    if (action.type === "underground-view-level") selectedUndergroundLevel = action.level;
    else undergroundLevel = undergroundLevel === undefined ? selectedUndergroundLevel : undefined;
    if (undergroundLevel !== undefined) undergroundLevel = selectedUndergroundLevel;
    landscapingPanel.setUndergroundView(undergroundLevel !== undefined, selectedUndergroundLevel);
    gameTest.ready = false;
  } else if (landscapingTool.state().active && (action.type === "hover" || action.type === "select" || action.type === "drag-start" || action.type === "drag-move" || action.type === "drag-end" || action.type === "drag-cancel")) {
    if (action.type === "drag-cancel") landscapingTool.handle(action);
    else {
      const pick = action.position ? renderer.pickSurface(action.position.x, action.position.y) : null;
      let tile: { x: number; y: number; height?: number; corner: "north_west" | "north_east" | "south_east" | "south_west" } | null = null;
      if (pick && world && action.position && (pick.kind === "terrain" || pick.kind === "carved-floor" || pick.kind === "path")) {
        const names = ["north_west", "north_east", "south_east", "south_west"] as const;
        const offsets = [[0, 0], [1, 0], [1, 1], [0, 1]];
        const surfaceOffset = (pick.y * world.dimensions[0] + pick.x) * 6;
        const distances = offsets.map(([dx, dy], index) => {
          const height = pick.kind === "terrain" && undergroundLevel === undefined ? world!.surfaces[surfaceOffset + index] : pick.height;
          const point = projectWorld(screenCamera(), viewport, { x: pick.x + dx, y: pick.y + dy, z: height / 4 });
          return Math.hypot(point.x - action.position!.x, point.y - action.position!.y);
        });
        const corner = distances.indexOf(Math.min(...distances));
        tile = { x: pick.x, y: pick.y, ...(undergroundLevel !== undefined ? { height: Math.round(pick.height) } : {}), corner: names[corner] };
      }
      landscapingTool.handle({ type: action.type, tile });
    }
  } else if (sceneryTool.state().mode !== "inactive" && (action.type === "hover" || action.type === "select")) {
    const position = action.position;
    const surface = position ? renderer.pickSurface(position.x, position.y) : null;
    let tile = null;
    if (surface && position && surface.kind === "terrain") {
      const offsets = [[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]];
      const distances = offsets.map(([dx, dy]) => {
        const point = projectWorld(screenCamera(), viewport, { x: surface.x + dx, y: surface.y + dy, z: surface.height / 4 });
        return Math.hypot(point.x - position.x, point.y - position.y);
      });
      tile = { x: surface.x, y: surface.y, z: Math.round(surface.height), quarter: distances.indexOf(Math.min(...distances)) };
    }
    if (action.type === "hover") sceneryTool.hover(tile);
    else {
      const selection = renderer.pick(action.position.x, action.position.y);
      const match = selection && "id" in selection ? /^scenery-(.+)$/.exec(selection.id) : null;
      sceneryTool.select(tile, match ? match[1] : undefined);
    }
  } else if (action.type === "path-build-mode" || action.type === "path-remove-mode") {
    openTool("paths");
    pathsElement.querySelector<HTMLButtonElement>(action.type === "path-build-mode" ? "[data-build]" : "[data-remove]")!.click();
  } else if (pathTool.state().mode !== "inactive" && (action.type === "hover" || action.type === "select" || action.type === "drag-start" || action.type === "drag-move" || action.type === "drag-end" || action.type === "drag-cancel")) {
    if (action.type === "drag-cancel") pathTool.handle(action);
    else {
      const pick = action.position && world ? renderer.pickSurface(action.position.x, action.position.y) : null;
      const tile = world ? pathTileFromPick(pick, world) : null;
      pathHover = tile;
      pathTool.handle({ type: action.type, tile });
    }
  } else if (action.type === "station-placement") {
    if (action.enabled) { openTool("coaster"); deleteMode = false; panel.setDeleteMode(false); }
    stationPlacement = action.enabled;
    station = undefined;
    gameCanvas.dataset.stationPreview = "";
    gameTest.ready = false;
  } else if (action.type === "station-height") {
    if (station) station.z += action.height - stationHeight;
    stationHeight = action.height;
    gameTest.ready = false;
  } else if (stationPlacement && action.type === "hover") {
    if (stationPlacement && action.position) updateStationPreview(action.position);
    else if (stationPlacement && !station?.locked) { station = undefined; gameCanvas.dataset.stationPreview = ""; gameTest.ready = false; }
  } else if (facilityPlacement && action.type === "hover") {
    const surface = action.position ? renderer.pickSurface(action.position.x, action.position.y) : null;
    const requestId = ++facilityPreviewRequestId;
    if (surface && (surface.kind === "terrain" || surface.kind === "carved-floor")) {
      sendWorkerAction(worker, { type: "facility-access-preview", requestId, kind: facilityPlacement.kind, x: surface.x, y: surface.y, height: Math.round(surface.height), rotation: facilityPlacement.rotation });
    } else facilityPanel.previewDirection(undefined);
  } else if (action.type === "select" && facilityRemoval) {
    const selection = renderer.pick(action.position.x, action.position.y);
    const match = selection && "id" in selection ? /^facility-(\d+)$/.exec(selection.id) : null;
    if (match) sendWorkerAction(worker, { type: "facility-remove", id: Number(match[1]) });
    else facilityPanel.message("Choose a facility to remove.");
  } else if (action.type === "select" && facilityPlacement) {
    const selection = renderer.pick(action.position.x, action.position.y);
    if (selection?.kind === "terrain" || selection?.kind === "carved-floor") {
      const height = Math.round(selection.height);
      sendWorkerAction(worker, { type: "facility-place", ...facilityPlacement, x: selection.x, y: selection.y, height });
    }
    else facilityPanel.message("Choose a flat terrain tile.");
  } else if (action.type === "select" && stationPlacement) {
    updateStationPreview(action.position);
    if (station?.locked) {
      sendWorkerAction(worker, { type: "construction-start", x: station.x, y: station.y, z: station.z, heading: station.heading });
      stationPlacement = false;
      station = undefined;
      gameCanvas.dataset.stationPreview = "";
    } else if (station) station.locked = true;
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
  } else if (action.type === "select") {
    const selection = renderer.pick(action.position.x, action.position.y);
    gameCanvas.dataset.selection = JSON.stringify(selection);
    const block = blockAt(action.position);
    if (block !== null) {
      selectBlock(block);
    } else if (selection && "id" in selection && selection.kind === "testing") {
      if (selectedBlock !== null) selectBlock(null);
      const eventMatch = /^proximity-(\d+)$/.exec(selection.id);
      const sampleMatch = /^testing-(\d+)$/.exec(selection.id);
      if (eventMatch) testingPanel.inspectProximity(Number(eventMatch[1]));
      else if (sampleMatch) testingPanel.inspect(Number(sampleMatch[1]));
    } else if (deleteMode && selection && "id" in selection) {
      const match = /^construction-track-(\d+)$/.exec(selection.id);
      if (match) dispatch({ type: "construction-delete", id: Number(match[1]) });
    } else if (selection && "id" in selection && /^guest-\d+$/.test(selection.id)) {
      selectedGuestId = selection.id;
      sendWorkerAction(worker, { type: "guest-detail-request", guestId: selection.id });
      gameTest.ready = false;
    }
  } else if (action.type === "chain-toggle") {
    if (construction?.active) sendWorkerAction(worker, { type: "construction-set-chain", enabled: !construction.chain });
  } else if (action.type === "construction-delete-mode" || action.type === "delete-mode-toggle") {
    deleteMode = action.type === "delete-mode-toggle" ? !deleteMode : action.enabled;
    panel.setDeleteMode(deleteMode);
    if (deleteMode && stationPlacement) dispatch({ type: "station-placement", enabled: false });
  } else if (action.type === "builder-cancel") {
    // Escape first leaves the active input mode, then closes the open tool. An open guest panel takes Escape for itself.
    if (selectedBlock !== null && activeTool === "coaster") selectBlock(null);
    else if (inputModeActive()) endInputModes();
    else if (!selectedGuestId) openTool("none");
  } else if (action.type === "piece-step" || action.type === "piece-group") {
    panel.choose(action);
  } else if (action.type === "construction-set-model") {
    sendWorkerAction(worker, { type: "construction-select", piece: `model:${action.model}` });
  } else if (isConstructionAction(action)) {
    sendWorkerAction(worker, action);
  } else if (action.type === "pan" || action.type === "zoom" || action.type === "rotate") {
    moveCamera(action);
    if (action.type !== "rotate") followPoints = undefined;
  }
}
function isConstructionAction(action: InputAction): action is Extract<InputAction, WorkerAction> {
  return action.type.startsWith("construction-");
}
bindInput(gameCanvas, dispatch, () => pathTool.state().mode === "build" || landscapingTool.state().active, (position) => blockAt(position) !== null, () => activeTool === "coaster" && selectedBlock !== null);

const worker = new Worker(new URL("./worker.ts", import.meta.url), {
  type: "module",
});
let workerReady = false;
let replayInProgress = false;
const params = new URLSearchParams(location.search);
const seedText = params.get("seed") ?? "2026";
if (!/^\d+$/.test(seedText) || BigInt(seedText) > 18446744073709551615n) throw new RangeError("Invalid seed");
const park = params.get("park") ?? "demo";
if (park !== "new" && park !== "demo") throw new RangeError("Invalid park");
const guestCount = testAgents === null ? 12 : Number(testAgents);
if (!Number.isInteger(guestCount) || guestCount < 1 || guestCount > 500) throw new RangeError("Invalid test guest count");
sendWorkerAction(worker, { type: "init", seed: BigInt(seedText), park, guestCount, frozen, manualBlockAnalysis });
worker.addEventListener("message", (event: MessageEvent<WorkerMessage>) => {
  const message = event.data;
  if (message.type === "ready") { workerReady = true;
  } else if (message.type === "session-export") {
    const inputs = pendingSessionExports.get(message.requestId);
    if (!inputs) throw new Error("Unknown session export request");
    pendingSessionExports.delete(message.requestId);
    const session = { format: "theme-park-session", version: 1, build: { gitHash: __GIT_HASH__, mode: import.meta.env.MODE }, ...message.recording, inputs };
    const url = URL.createObjectURL(new Blob([JSON.stringify(session, null, 2)], { type: "application/json" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `session-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
    link.click();
    URL.revokeObjectURL(url);
  } else if (message.type === "facility-access-preview") {
    if (facilityPlacement && message.requestId === facilityPreviewRequestId) facilityPanel.previewDirection(message.rotation);
  } else if (message.type === "scenery") {
    scenery = message.scenery;
    if (message.message) sceneryTool.receive(message.message);
    gameTest.ready = false;
  } else if (message.type === "terrain-result") {
    landscapingTool.receive(message);
    landscapingPanel.message(message.message);
  } else if (message.type === "path-preview" || message.type === "path-command-result") {
    pathTool.receive(message);
  } else if (message.type === "world") {
    const coasterGeometry = message.coasterGeometry ?? world?.coasterGeometry;
    const coasterRails = message.coasterRails ?? world?.coasterRails;
    const coasterTies = message.coasterTies ?? world?.coasterTies;
    const coasterColors = message.coasterColors !== undefined ? message.coasterColors : world?.coasterColors;
    const coasterTrain = message.coasterTrain !== undefined ? message.coasterTrain : world?.coasterTrain;
    if (!coasterGeometry || !coasterRails || !coasterTies || coasterColors === undefined || coasterTrain === undefined) throw new Error("Initial coaster geometry is missing");
    world = { ...message, coasterGeometry, coasterRails, coasterTies, coasterColors, coasterTrain };
    testingPanel.setWorld(world);
    landscapingTool.setDimensions(world.dimensions[0], world.dimensions[1]);
    gameTest.ready = false;
  } else if (message.type === "test-advanced") {
    advanceTicksComplete?.();
    advanceTicksComplete = undefined;
  } else if (message.type === "test-block-analysis-advanced") {
    advanceBlockAnalysisComplete?.();
    advanceBlockAnalysisComplete = undefined;
  } else if (message.type === "facilities") {
    if (frozen && facilities.length && !message.message && !message.forced) return;
    facilities = message.facilities;
    const panelSnapshot = JSON.stringify([facilities.map(({ id, kind, price }) => ({ id, kind, price })), message.revenue]);
    if (panelSnapshot !== facilityPanelSnapshot) {
      facilityPanel.update(facilities, message.revenue);
      facilityPanelSnapshot = panelSnapshot;
    }
    if (message.placed) facilityPlacement = undefined;
    if (message.message) facilityPanel.message(message.message);
    gameTest.ready = false;
  } else if (message.type === "piece-shapes") {
    panel.setShapes(message.shapes);
  } else if (message.type === "construction-state" || message.type === "block-analysis-progress") {
    if (message.type === "block-analysis-progress" && !construction) throw new Error("Block analysis progress arrived before construction state");
    const next: ConstructionState = message.type === "construction-state" ? message : { ...construction!, ...message, type: "construction-state" };
    const redraw = message.type === "construction-state" || blockLayoutChanged(construction!.blockReport.sections, next.blockReport.sections);
    if (!frozen) constructionEffects.observe(construction, next, performance.now());
    const preview = railPoints(next.previewRails);
    const previewKey = JSON.stringify([preview[0], preview.at(-1)]);
    if (pathTool.state().mode === "inactive" && preview.length && previewKey !== followedPreview) followPoints = preview;
    followedPreview = previewKey;
    construction = next;
    panel.update(next);
    if (selectedBlock !== null && !next.blockReport.sections.some((section) => section.editableIndex === selectedBlock)) selectBlock(null);
    gameCanvas.dataset.blockBoundaries = String(next.blockReport.sections.length);
    const reportJson = JSON.stringify(next.testingReport ?? null);
    if (reportJson !== testingReportJson) { testingReportJson = reportJson; testingPanel.update(next.testingReport ?? null); }
    if (redraw) gameTest.ready = false;
  } else if (message.type === "guest-detail") {
    latestGuestDetail = message.detail;
    if (message.guestId === selectedGuestId && message.detail) guestPanel.update(message.detail);
  } else if (message.type === "snapshot") {
    if (frameLogEnabled) snapshotArrivalEntries.push({ now: performance.now(), sentAt: message.sentAt, tickCount: message.tickCount });
    if (!frozen && !renderClock.push(message)) return;
    previous = current;
    if (!frozen || !current || current.guestIds.length === 0 || advanceTicksComplete) current = message;
    if (!frozen) observeTick(message);
    else if (advanceTicksComplete) observeCrests(message);
    output.value = String(message.tickCount);
    gameCanvas.dataset.agentCount = String(current?.guestIds.length ?? 0);
  } else if (message.type === "surface-edited" || message.type === "test-resumed" || message.type === "construction-finish-ready") {
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
    gameTest.ready = false;
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
  const demo = detectCrest(demoCrests, snapshot.trainPoses, snapshot.tickCount);
  const built = detectCrest(constructionCrests, snapshot.constructionTrainPoses, snapshot.tickCount);
  demoCrests = demo.tracker; constructionCrests = built.tracker;
  const newBursts = [demo.burst, built.burst].filter((burst) => burst !== undefined);
  crestHistory.push(...newBursts);
  crestBursts = [...crestBursts, ...newBursts].filter((burst) => snapshot.tickCount - burst.tick < CONFETTI_LIFE_TICKS);
}

function frame(now: number): void {
  if (!world || replayInProgress) return;
  if (frozen && gameTest.ready) return;
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
  const heights = activeTool === "coaster" && construction?.active ? constructionHeights(world, construction.placedRails) : null;
  heightMarkers.hidden = heights === null;
  if (heights) {
    const trackText = `Track end: ${formatLength(heights.end.metres, unitsSetting.units())} above ground`;
    const stationText = `Station: ${formatLength(heights.station.metres, unitsSetting.units())} above ground`;
    trackHeightMarker.textContent = trackText;
    stationHeightMarker.textContent = stationText;
    panel.setHeightMarkers(trackText, stationText);
    const anchor = projectWorld(camera, cssViewport(), heights.end.position);
    heightMarkers.style.left = `${anchor.x}px`;
    heightMarkers.style.top = `${anchor.y}px`;
  }
  constructionEffects.active(now);
  const occupants = renderCurrent ? riderAppearances(renderCurrent, renderCurrent.constructionRiders, atlas) : new Map();
  const ridingIds = new Set([...occupants.values()].map((occupant) => occupant.id));
  const people = renderCurrent ? buildPeople(renderPrevious, renderCurrent, interpolation, camera.rotation, atlas).filter((person) => !ridingIds.has(person.id)) : [];
  renderedGuests = people;
  const selectedGuest = people.find((guest) => guest.id === selectedGuestId);
  if (selectedGuest) guestPanel.setAnchor(projectWorld(screenCamera(), viewport, { x: selectedGuest.x, y: selectedGuest.y, z: selectedGuest.height / 4 }));
  const trainPoses = renderCurrent ? interpolateTrainPoses(renderPrevious?.trainPoses ?? renderCurrent.trainPoses, renderCurrent.trainPoses, interpolation) : new Float32Array();
  const constructionTrainPoses = renderCurrent ? interpolateTrainPoses(renderPrevious?.constructionTrainPoses ?? renderCurrent.constructionTrainPoses, renderCurrent.constructionTrainPoses, interpolation) : noGeometry;
  const sprites = [...facilitySprites(facilities, camera.rotation), ...scenerySprites(scenery, camera.rotation), ...people];
  const view = { ...world, sprites, trainPoses, undergroundLevel };
  const currentWorld = world;
  dayClock.update(dayPhase);
  // Test mode holds the simulation still, so effects there are timed from the time of day alone.
  const effectTick = frozen ? Math.round(dayPhase * DAY_LENGTH_TICKS) : renderFrame?.tick ?? 0;
  const backdrop = BACKDROPS[themeToggle.theme()];
  const { lighting, paper } = printOnBackdrop(lightingAt(dayPhase), backdrop);
  const sun = sunAt(dayPhase);
  const phase = dayPhase;
  const atmosphere: AtmospherePass = (renderCamera, renderViewport, renderAtlas, scale) => {
    const rails = construction ? [currentWorld.coasterRails, construction.placedRails] : [currentWorld.coasterRails];
    const lights = buildLights({ facilities, rails, sprites, atlas: renderAtlas, scale }, lighting, effectTick, renderCamera, renderViewport);
    const fireworks = buildFireworks(parkBounds(facilities, rails), lighting, effectTick, renderCamera, renderViewport);
    const horizonY = projectWorld(renderCamera, renderViewport, { x: currentWorld.dimensions[0] / 2, y: currentWorld.dimensions[1] / 2, z: 0 }).y;
    return {
      lighting, horizonY, paper,
      sky: buildSky(lighting, backdrop, phase, horizonY, renderCamera, renderViewport, renderAtlas, scale),
      decals: buildShadows(facilities, lighting, sun, renderCamera, renderViewport),
      lights: lights.bulbs,
      pools: lights.pools,
      overlay: buildConfetti(crestBursts, effectTick, renderCamera, renderViewport),
      sparks: fireworks.sparks,
      glows: [...lights.glows, ...fireworks.glows],
    };
  };
  renderedRiders = [];
  renderer.render(view, screenCamera(), frozen ? 0 : Math.floor(now / 250) % 4, [
    (renderCamera, renderViewport) => buildSceneryGhostQuads(sceneryTool.state().preview, view, renderCamera, renderViewport),
    (renderCamera, renderViewport) => buildLandscapingGhostQuads(landscapingTool.state().preview, view, renderCamera, renderViewport),
    (renderCamera, renderViewport) => pathTool.state().preview ? buildPathGhostQuads(decodePathPreview(pathTool.state().preview!.paths), renderCamera, renderViewport) : [],
    (renderCamera, renderViewport, renderAtlas) => buildFacilityGeometry(facilities, renderCamera, renderViewport, trackStyle(renderAtlas)),
    (renderCamera, renderViewport, renderAtlas) => buildSelectionRing(selectedGuest, renderCamera, renderViewport, renderAtlas),
    (renderCamera, renderViewport, renderAtlas, scale) => [
      ...(currentWorld.coasterColors ? staticTrack(currentWorld.coasterRails, currentWorld.coasterTies, currentWorld.coasterSupports, currentWorld.coasterColors, renderCamera, renderViewport) : []),
      ...(construction?.colors ? (activeTool === "coaster" && construction.blockReport.sections.length
        ? buildTrackQuads(construction.placedRails, construction.placedTies, construction.placedSupports, renderCamera, renderViewport, trackPaint(trackStyle(renderAtlas), construction.colors), construction.blockReport.sections.map((section) => section.boundaryMetres), construction.railDistances)
        : builtTrack(construction.placedRails, construction.placedTies, construction.placedSupports, construction.colors, renderCamera, renderViewport)).map((quad) => ({ ...quad, id: `construction-${quad.id}` })) : []),
      ...(construction && activeTool === "coaster" ? buildBlockBoundaryQuads(construction.placedRails, construction.railDistances, construction.blockReport.sections, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...buildGhostQuads(stationGhost(), noGeometry, noGeometry, true, renderCamera, renderViewport, trackStyle(renderAtlas)).map((quad) => ({ ...quad, color: [0.25, 0.55, 0.85, 0.55] as const })),
      ...(construction ? buildGhostQuads(construction.previewRails, construction.previewTies, construction.previewSupports, construction.canAppend, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...(currentWorld.coasterColors && currentWorld.coasterTrain ? buildTrainQuads(trainPoses, renderCamera, renderViewport, trackPaint(trackStyle(renderAtlas), currentWorld.coasterColors), currentWorld.coasterTrain) : []),
      ...(construction?.colors && construction.trainLayout ? buildTrainQuads(constructionTrainPoses, renderCamera, renderViewport, trackPaint(trackStyle(renderAtlas), construction.colors), construction.trainLayout) : []),
      ...(currentWorld.coasterTrain ? buildRiderQuads(trainPoses, renderCamera, renderViewport, renderAtlas, scale, currentWorld.coasterTrain) : []),
      ...(construction?.trainLayout ? buildRiderQuads(constructionTrainPoses, renderCamera, renderViewport, renderAtlas, scale, construction.trainLayout, occupants) : []).map((quad) => {
        renderedRiders.push({ id: quad.id, left: Math.min(...quad.vertices.map((vertex) => vertex.x)), right: Math.max(...quad.vertices.map((vertex) => vertex.x)), top: Math.min(...quad.vertices.map((vertex) => vertex.y)), bottom: Math.max(...quad.vertices.map((vertex) => vertex.y)) });
        if (quad.id === selectedGuestId) {
          guestPanel.setAnchor({ x: quad.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 4, y: quad.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 4 });
        }
        return quad;
      }),
      ...(testingMetric && construction?.testingReport ? buildTestingRibbon(construction.testingReport.samples, testingMetric, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...(construction?.testingReport ? buildProximityMarkers(construction.testingReport.proximityEvents, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...constructionEffects.quads(now, renderCamera, renderViewport),
    ],
  ], atmosphere);
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
  gameCanvas.dataset.drawnAgents = String(people.length);
  gameTest.ready = renderer.isContextAvailable() && (current?.guestIds.length ?? 0) > 0;
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
