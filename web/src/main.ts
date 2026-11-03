import { buildFacilityGeometry, facilitySprites, type Facility, type FacilityKind } from "./facilities";
import { createFacilityPanel } from "./facility-panel";
import { applyCameraAction, projectWorld, type Camera, type WorldPoint } from "./camera";
import { bindInput, bindKeyboard, builderShortcuts, keyHint } from "./input/bindings";
import type { InputAction } from "./input/actions";
import type { GameTest } from "./game-test";
import { createWorldRenderer, loadAtlas, type WorldView, type WorldSprite } from "./render/index";
import { buildPeople, riderAppearances, type GuestSnapshot } from "./render/people";
import { buildRiderQuads, buildTrackQuads, interpolateTrainPoses, trackStyle } from "./render/track/geometry";
import { createStaticTrackCache } from "./render/track/cache";
import type { WorldSnapshot } from "./world-snapshot";
import { createConstructionPanel } from "./construction";
import type { ConstructionState, PieceShapesMessage } from "./construction-protocol";
import { followTrackEnd, railPoints } from "./construction-camera";
import { createConstructionEffects } from "./construction-effects";
import { buildGhostQuads } from "./render/track/ghost";
import { createGuestPanel } from "./guest-panel/guest-panel";
import type { GuestDetail } from "./guest-panel/protocol";
import { buildSelectionRing } from "./guest-panel/selection-ring";
import { createPathTool, type PathToolMessage } from "./path-tool";
import { createPathPanel } from "./path-panel";
import { pathTileFromPick } from "./path-picking";
import { buildPathGhostQuads, decodePathPreview, pathPreviewRefusalAt } from "./render/path-ghost";
import { buildTestingRibbon, createTestingPanel, type TestingMetric } from "./coaster-testing";
import { buildLandscapingGhostQuads, createLandscapingTool, type TerrainEditResult } from "./landscaping-tool";
import { createLandscapingPanel } from "./landscaping-panel";

const canvas = document.querySelector<HTMLCanvasElement>("canvas");
const tickOutput = document.querySelector<HTMLOutputElement>("[data-testid='tick-count']");
if (!canvas || !tickOutput) throw new Error("Game page elements are missing");
const gameCanvas = canvas;
const output = tickOutput;
const viewport = { width: gameCanvas.width, height: gameCanvas.height };
let camera: Camera = { centerX: 250, centerY: 256, zoom: 1, rotation: 0 };
type CoasterWorld = WorldView & { pathMetadata: Int32Array; coasterGeometry: Float32Array; coasterRails: Float32Array; coasterTies: Float32Array; coasterSupports: Float32Array };
type CoasterWorldMessage = WorldSnapshot & { coasterGeometry?: Float32Array; coasterRails?: Float32Array; coasterTies?: Float32Array; coasterSupports: Float32Array };
type CoasterSnapshot = GuestSnapshot & { tickCount: number; trainPoses: Float32Array; constructionTrainPoses: Float32Array; constructionRiders: Uint32Array };
let world: CoasterWorld | undefined;
let previous: CoasterSnapshot | undefined;
let current: CoasterSnapshot | undefined;
let receivedAt = performance.now();
const noGeometry = new Float32Array();
const frozen = new URLSearchParams(location.search).has("test");
const timing = new URLSearchParams(location.search).get("timing") === "work";
const { atlas, images } = await loadAtlas(new URL("./atlas.json", document.baseURI).href);
const renderer = createWorldRenderer(gameCanvas, atlas, images);
const staticTrack = createStaticTrackCache(atlas);
const builtTrack = createStaticTrackCache(atlas);
let construction: ConstructionState | undefined;
let deleteMode = false;
let stationPlacement = false;
let stationHeight = 12;
let station: { x: number; y: number; z: number; heading: number; locked: boolean } | undefined;
const stationDirections = [[1, 0], [0, 1], [-1, 0], [0, -1]] as const;
function updateStationPreview(position: { x: number; y: number }): void {
  if (station?.locked) {
    const origin = projectWorld(camera, viewport, { x: station.x + 0.5, y: station.y + 0.5, z: (station.z - stationHeight) / 4 });
    const delta = { x: position.x - origin.x, y: position.y - origin.y };
    if (Math.hypot(delta.x, delta.y) > 4) {
      let best = -Infinity;
      stationDirections.forEach(([dx, dy], index) => {
        const end = projectWorld(camera, viewport, { x: station!.x + 0.5 + dx, y: station!.y + 0.5 + dy, z: (station!.z - stationHeight) / 4 });
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
bindKeyboard(document.body, dispatch, () => ({ x: gameCanvas.width / 2, y: gameCanvas.height / 2 }));
const constructionEffects = createConstructionEffects(trackStyle(atlas));
let followPoints: WorldPoint[] | undefined;
let followedPreview = "";
let lastFrameAt = performance.now();
let testingMetric: TestingMetric | null = null;
let testingReportJson = "";
const testingPanel = createTestingPanel(panelContainer.querySelector<HTMLElement>("[data-testing]")!, trackStyle(atlas), (metric) => { testingMetric = metric; gameTest.ready = false; });
if (!atlas.themeStyle) throw new Error("Atlas theme style is missing");
let selectedGuestId: string | undefined;
const guestPanel = createGuestPanel(gameCanvas, atlas.themeStyle, {
  onSelectGuestId(id) {
    selectedGuestId = id;
    worker.postMessage({ type: "guest-detail-request", guestId: id });
    gameTest.ready = false;
  },
  onClose() {
    selectedGuestId = undefined;
    worker.postMessage({ type: "guest-detail-close" });
    gameTest.ready = false;
  },
});

let facilities: Facility[] = [];
let facilityPanelSnapshot = "";
let facilityPlacement: { kind: FacilityKind; rotation: number } | undefined;
let facilityRemoval = false;
let advanceTicksComplete: (() => void) | undefined;
const facilityContainer = document.querySelector<HTMLElement>("[data-facilities]");
if (!facilityContainer) throw new Error("Facility panel is missing");
const facilityPanel = createFacilityPanel(facilityContainer, (kind, rotation) => {
  landscapingTool.cancel();
  facilityPlacement = { kind, rotation };
  facilityRemoval = false;
  stationPlacement = false;
  pathTool.cancel();
  deleteMode = false; panel.setDeleteMode(false);
}, () => { landscapingTool.cancel(); facilityRemoval = true; facilityPlacement = undefined; stationPlacement = false; pathTool.cancel(); deleteMode = false; panel.setDeleteMode(false); },
() => { facilityPlacement = undefined; facilityRemoval = false; },
(id, price) => worker.postMessage({ type: "facility-price", id, price }));
const pathContainer = document.querySelector<HTMLElement>("[data-paths]");
if (!pathContainer) throw new Error("Path panel is missing");
const pathsElement = pathContainer;
let pathHover: { x: number; y: number } | null = null;
const pathTool = createPathTool((command) => worker.postMessage(command), () => {
  const state = pathTool.state();
  const refusal = state.preview && pathHover ? pathPreviewRefusalAt(decodePathPreview(state.preview.paths), pathHover.x, pathHover.y) : null;
  pathPanel.message(refusal ?? state.message);
  gameTest.ready = false;
});
const pathPanel = createPathPanel(pathContainer, (selection) => {
  landscapingTool.cancel();
  facilityPlacement = undefined; facilityRemoval = false; stationPlacement = false;
  deleteMode = false; panel.setDeleteMode(false);
  followPoints = undefined;
  pathTool.build(selection);
}, () => {
  landscapingTool.cancel();
  facilityPlacement = undefined; facilityRemoval = false; stationPlacement = false;
  deleteMode = false; panel.setDeleteMode(false);
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
  worker.postMessage(command);
}, () => { gameTest.ready = false; });
let latestGuestDetail: GuestDetail | null = null;
let renderedGuests: readonly WorldSprite[] = [];
const gameTest: GameTest = {
  ready: false,
  guests() { return renderedGuests; },
  riders() {
    const riders = current?.constructionRiders ?? new Uint32Array();
    const result: { id: string; car: number; seat: number; x: number; y: number; z: number }[] = [];
    for (let offset = 0; offset < riders.length; offset += 3) {
      const car = riders[offset];
      const poseOffset = car * 11;
      const poses = current!.constructionTrainPoses;
      if (poses[poseOffset] !== car) throw new Error("Missing rider car pose");
      result.push({ car, seat: riders[offset + 1], id: `guest-${riders[offset + 2]}`, x: poses[poseOffset + 1], y: poses[poseOffset + 2], z: poses[poseOffset + 3] });
    }
    return result;
  },
  facilities() { return facilities; },
  paths() {
    if (!world) return [];
    const paths = [];
    for (let offset = 0; offset < world.paths.length; offset += 4) {
      const [x, y, z, slope] = world.paths.subarray(offset, offset + 4);
      const [kind, style] = world.pathMetadata.subarray(offset / 4 * 3, offset / 4 * 3 + 2);
      paths.push({ x, y, z, slope, kind, style });
    }
    return paths;
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
  requestGuestDetail(id) { worker.postMessage({ type: "guest-detail-request", guestId: id }); },
  guestDetail() { return latestGuestDetail; },
  testingReport() { return construction?.testingReport ?? null; },
  advanceTicks(count) {
    if (!frozen || advanceTicksComplete || !Number.isInteger(count) || count < 1 || count > 100000) throw new Error("Invalid test tick advance");
    return new Promise<void>((resolve) => { advanceTicksComplete = resolve; worker.postMessage({ type: "test-advance", count }); });
  },
  setCamera(value) { camera = value; followPoints = undefined; gameTest.ready = false; },
  setSurface(x, y, corners) { worker.postMessage({ type: "set-surface", x, y, corners: new Int16Array(corners) }); },
  project(point) { return projectWorld(camera, viewport, point); },
  pick(x, y) { return renderer.pick(x, y); },
  pickSurface(x, y) { return renderer.pickSurface(x, y); },
};
window.gameTest = gameTest;

function dispatch(action: InputAction): void {
  if (action.type === "landscaping-activate") {
    pathTool.cancel(); facilityPlacement = undefined; facilityRemoval = false; stationPlacement = false;
    deleteMode = false; panel.setDeleteMode(false); followPoints = undefined;
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
          const point = projectWorld(camera, viewport, { x: pick.x + dx, y: pick.y + dy, z: height / 4 });
          return Math.hypot(point.x - action.position!.x, point.y - action.position!.y);
        });
        const corner = distances.indexOf(Math.min(...distances));
        tile = { x: pick.x, y: pick.y, ...(undergroundLevel !== undefined ? { height: Math.round(pick.height) } : {}), corner: names[corner] };
      }
      landscapingTool.handle({ type: action.type, tile });
    }
  } else if (action.type === "path-build-mode" || action.type === "path-remove-mode") {
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
    if (action.enabled) { landscapingTool.cancel(); pathTool.cancel(); deleteMode = false; panel.setDeleteMode(false); }
    facilityPlacement = undefined;
    facilityRemoval = false;
    stationPlacement = action.enabled;
    station = undefined;
    gameCanvas.dataset.stationPreview = "";
    gameTest.ready = false;
  } else if (action.type === "station-height") {
    if (station) station.z += action.height - stationHeight;
    stationHeight = action.height;
    gameTest.ready = false;
  } else if (action.type === "hover") {
    if (stationPlacement && action.position) updateStationPreview(action.position);
    else if (stationPlacement && !station?.locked) { station = undefined; gameCanvas.dataset.stationPreview = ""; gameTest.ready = false; }
  } else if (action.type === "select" && facilityRemoval) {
    const selection = renderer.pick(action.position.x, action.position.y);
    const match = selection && "id" in selection ? /^facility-(\d+)$/.exec(selection.id) : null;
    if (match) worker.postMessage({ type: "facility-remove", id: Number(match[1]) });
    else facilityPanel.message("Choose a facility to remove.");
  } else if (action.type === "select" && facilityPlacement) {
    const selection = renderer.pick(action.position.x, action.position.y);
    if (selection?.kind === "terrain" || selection?.kind === "carved-floor") worker.postMessage({ type: "facility-place", ...facilityPlacement, x: selection.x, y: selection.y, ...(selection.kind === "carved-floor" ? { height: Math.round(selection.height) } : {}) });
    else facilityPanel.message("Choose a flat terrain tile.");
  } else if (action.type === "select" && stationPlacement) {
    updateStationPreview(action.position);
    if (station?.locked) {
      worker.postMessage({ type: "construction-start", x: station.x, y: station.y, z: station.z, heading: station.heading });
      stationPlacement = false;
      station = undefined;
      gameCanvas.dataset.stationPreview = "";
    } else if (station) station.locked = true;
  } else if (action.type === "select") {
    const selection = renderer.pick(action.position.x, action.position.y);
    gameCanvas.dataset.selection = JSON.stringify(selection);
    if (selection && "id" in selection && selection.kind === "testing") {
      const match = /^testing-(\d+)$/.exec(selection.id);
      if (match) testingPanel.inspect(Number(match[1]));
    } else if (deleteMode && selection && "id" in selection) {
      const match = /^construction-track-(\d+)$/.exec(selection.id);
      if (match) dispatch({ type: "construction-delete", id: Number(match[1]) });
    } else if (selection && "id" in selection && /^guest-\d+$/.test(selection.id)) {
      selectedGuestId = selection.id;
      worker.postMessage({ type: "guest-detail-request", guestId: selection.id });
      gameTest.ready = false;
    }
  } else if (action.type === "construction-delete-mode") {
    deleteMode = action.enabled;
    if (deleteMode) { landscapingTool.cancel(); pathTool.cancel(); facilityPlacement = undefined; facilityRemoval = false; stationPlacement = false; }
  } else if (action.type === "delete-mode-toggle" || action.type === "builder-cancel") {
    deleteMode = action.type === "delete-mode-toggle" && !deleteMode;
    if (deleteMode) { landscapingTool.cancel(); pathTool.cancel(); facilityPlacement = undefined; facilityRemoval = false; stationPlacement = false; }
    if (action.type === "builder-cancel") { landscapingTool.cancel(); pathTool.cancel(); facilityPlacement = undefined; facilityRemoval = false; }
    panel.setDeleteMode(deleteMode);
    if (action.type === "builder-cancel" && stationPlacement) dispatch({ type: "station-placement", enabled: false });
  } else if (action.type === "piece-step" || action.type === "piece-group") {
    panel.choose(action);
  } else if (action.type.startsWith("construction-")) {
    worker.postMessage(action);
  } else if (action.type === "pan" || action.type === "zoom" || action.type === "rotate") {
    camera = applyCameraAction(camera, viewport, action);
    if (action.type !== "rotate") followPoints = undefined;
  }
}
bindInput(gameCanvas, dispatch, () => pathTool.state().mode === "build" || landscapingTool.state().active);

const worker = new Worker(new URL("./worker.ts", import.meta.url), {
  type: "module",
  name: frozen ? "theme-park-test" : "theme-park",
});
worker.addEventListener("message", (event: MessageEvent<CoasterWorldMessage | ConstructionState | ({ type: "snapshot"; tickCount: number } & CoasterSnapshot) | { type: "ready" } | { type: "test-advanced" } | { type: "facilities"; facilities: Facility[]; message: string; revenue: string; placed: boolean; forced: boolean } | { type: "guest-detail"; guestId: string; detail: GuestDetail | null } | PieceShapesMessage | PathToolMessage | TerrainEditResult>) => {
  const message = event.data;
  if (message.type === "terrain-result") {
    landscapingTool.receive(message);
    landscapingPanel.message(message.message);
  } else if (message.type === "path-preview" || message.type === "path-command-result") {
    pathTool.receive(message);
  } else if (message.type === "world") {
    const coasterGeometry = message.coasterGeometry ?? world?.coasterGeometry;
    const coasterRails = message.coasterRails ?? world?.coasterRails;
    const coasterTies = message.coasterTies ?? world?.coasterTies;
    if (!coasterGeometry || !coasterRails || !coasterTies) throw new Error("Initial coaster geometry is missing");
    world = { ...message, coasterGeometry, coasterRails, coasterTies };
    landscapingTool.setDimensions(world.dimensions[0], world.dimensions[1]);
    gameTest.ready = false;
  } else if (message.type === "test-advanced") {
    advanceTicksComplete?.();
    advanceTicksComplete = undefined;
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
  } else if (message.type === "construction-state") {
    if (!frozen) constructionEffects.observe(construction, message, performance.now());
    const preview = railPoints(message.previewRails);
    const previewKey = JSON.stringify([preview[0], preview.at(-1)]);
    if (pathTool.state().mode === "inactive" && preview.length && previewKey !== followedPreview) followPoints = preview;
    followedPreview = previewKey;
    construction = message;
    panel.update(message);
    const reportJson = JSON.stringify(message.testingReport ?? null);
    if (reportJson !== testingReportJson) { testingReportJson = reportJson; testingPanel.update(message.testingReport ?? null); }
    gameTest.ready = false;
  } else if (message.type === "guest-detail") {
    latestGuestDetail = message.detail;
    if (message.guestId === selectedGuestId && message.detail) guestPanel.update(message.detail);
  } else if (message.type === "snapshot") {
    previous = current;
    if (!frozen || !current || current.guestIds.length === 0 || advanceTicksComplete) current = message;
    receivedAt = performance.now();
    output.value = String(message.tickCount);
    gameCanvas.dataset.agentCount = String(current?.guestIds.length ?? 0);
  }
});

function frame(now: number): void {
  if (!world) return;
  if (frozen && gameTest.ready) return;
  const interpolation = frozen ? 1 : Math.max(0, Math.min(1, (now - receivedAt) / (1000 / 30)));
  if (followPoints) {
    const followed = followTrackEnd(camera, viewport, followPoints, frozen ? 1 : 1 - Math.exp(-(now - lastFrameAt) / 140));
    camera = followed.camera;
    if (followed.settled) followPoints = undefined;
  }
  lastFrameAt = now;
  constructionEffects.active(now);
  const occupants = current ? riderAppearances(current, current.constructionRiders, atlas) : new Map();
  const ridingIds = new Set([...occupants.values()].map((occupant) => occupant.id));
  const people = current ? buildPeople(previous, current, interpolation, camera.rotation, atlas).filter((person) => !ridingIds.has(person.id)) : [];
  renderedGuests = people;
  const selectedGuest = people.find((guest) => guest.id === selectedGuestId);
  if (selectedGuest) guestPanel.setAnchor(projectWorld(camera, viewport, { x: selectedGuest.x, y: selectedGuest.y, z: selectedGuest.height / 4 }));
  const trainPoses = current ? interpolateTrainPoses(previous?.trainPoses ?? current.trainPoses, current.trainPoses, interpolation) : new Float32Array();
  const constructionTrainPoses = current ? interpolateTrainPoses(previous?.constructionTrainPoses ?? current.constructionTrainPoses, current.constructionTrainPoses, interpolation) : noGeometry;
  const sprites = [...facilitySprites(facilities, camera.rotation), ...people];
  const view = { ...world, sprites, trainPoses, undergroundLevel };
  const currentWorld = world;
  renderer.render(view, camera, frozen ? 0 : Math.floor(now / 250) % 4, [
    (renderCamera, renderViewport) => buildLandscapingGhostQuads(landscapingTool.state().preview, view, renderCamera, renderViewport),
    (renderCamera, renderViewport) => pathTool.state().preview ? buildPathGhostQuads(decodePathPreview(pathTool.state().preview!.paths), renderCamera, renderViewport) : [],
    (renderCamera, renderViewport, renderAtlas) => buildFacilityGeometry(facilities, renderCamera, renderViewport, trackStyle(renderAtlas)),
    (renderCamera, renderViewport, renderAtlas) => buildSelectionRing(selectedGuest, renderCamera, renderViewport, renderAtlas),
    (renderCamera, renderViewport, renderAtlas, scale) => [
      ...staticTrack(currentWorld.coasterRails, currentWorld.coasterTies, currentWorld.coasterSupports, renderCamera, renderViewport),
      ...(construction ? builtTrack(construction.placedRails, construction.placedTies, construction.placedSupports, renderCamera, renderViewport).map((quad) => ({ ...quad, id: `construction-${quad.id}` })) : []),
      ...buildGhostQuads(stationGhost(), noGeometry, noGeometry, true, renderCamera, renderViewport, trackStyle(renderAtlas)).map((quad) => ({ ...quad, color: [0.25, 0.55, 0.85, 0.55] as const })),
      ...(construction ? buildGhostQuads(construction.previewRails, construction.previewTies, construction.previewSupports, construction.canAppend, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...buildTrackQuads(noGeometry, noGeometry, noGeometry, trainPoses, renderCamera, renderViewport, trackStyle(renderAtlas)),
      ...buildTrackQuads(noGeometry, noGeometry, noGeometry, constructionTrainPoses, renderCamera, renderViewport, trackStyle(renderAtlas)),
      ...buildRiderQuads(trainPoses, renderCamera, renderViewport, renderAtlas, scale),
      ...buildRiderQuads(constructionTrainPoses, renderCamera, renderViewport, renderAtlas, scale, occupants).map((quad) => {
        if (quad.id === selectedGuestId) {
          guestPanel.setAnchor({ x: quad.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 4, y: quad.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 4 });
        }
        return quad;
      }),
      ...(testingMetric && construction?.testingReport ? buildTestingRibbon(construction.testingReport.samples, testingMetric, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...constructionEffects.quads(now, renderCamera, renderViewport),
    ],
  ]);
  gameCanvas.dataset.drawnAgents = String(people.length);
  gameTest.ready = (current?.guestIds.length ?? 0) > 0;
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
