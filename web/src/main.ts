import { applyCameraAction, projectWorld, type Camera } from "./camera";
import { bindInput } from "./input/bindings";
import type { InputAction } from "./input/actions";
import type { GameTest } from "./game-test";
import { createWorldRenderer, loadAtlas, type WorldView, type WorldSprite } from "./render/index";
import { buildPeople, type GuestSnapshot } from "./render/people";
import { buildRiderQuads, buildTrackQuads, interpolateTrainPoses, trackStyle } from "./render/track/geometry";
import { createStaticTrackCache } from "./render/track/cache";
import type { WorldSnapshot } from "./world-snapshot";
import { createConstructionPanel } from "./construction";
import type { ConstructionState } from "./construction-protocol";
import { buildGhostQuads } from "./render/track/ghost";
import { createGuestPanel } from "./guest-panel/guest-panel";
import type { GuestDetail } from "./guest-panel/protocol";
import { buildSelectionRing } from "./guest-panel/selection-ring";

const canvas = document.querySelector<HTMLCanvasElement>("canvas");
const tickOutput = document.querySelector<HTMLOutputElement>("[data-testid='tick-count']");
if (!canvas || !tickOutput) throw new Error("Game page elements are missing");
const gameCanvas = canvas;
const output = tickOutput;
const viewport = { width: gameCanvas.width, height: gameCanvas.height };
let camera: Camera = { centerX: 250, centerY: 256, zoom: 1, rotation: 0 };
type CoasterWorld = WorldView & { coasterGeometry: Float32Array; coasterRails: Float32Array; coasterTies: Float32Array; coasterSupports: Float32Array };
type CoasterWorldMessage = WorldSnapshot & { coasterGeometry?: Float32Array; coasterRails?: Float32Array; coasterTies?: Float32Array; coasterSupports: Float32Array };
type CoasterSnapshot = GuestSnapshot & { trainPoses: Float32Array; constructionTrainPoses: Float32Array };
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
    if (selection?.kind === "terrain") station = { x: selection.x, y: selection.y, z: Math.ceil(selection.height / 2) * 2 + stationHeight, heading: 0, locked: false };
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

let renderedGuests: readonly WorldSprite[] = [];
const gameTest: GameTest = {
  ready: false,
  guests() { return renderedGuests; },
  setCamera(value) { camera = value; gameTest.ready = false; },
  setSurface(x, y, corners) { worker.postMessage({ type: "set-surface", x, y, corners: new Int16Array(corners) }); },
  project(point) { return projectWorld(camera, viewport, point); },
  pick(x, y) { return renderer.pick(x, y); },
};
window.gameTest = gameTest;

function dispatch(action: InputAction): void {
  if (action.type === "station-placement") {
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
    if (deleteMode && selection && "id" in selection) {
      const match = /^construction-track-(\d+)$/.exec(selection.id);
      if (match) dispatch({ type: "construction-delete", id: Number(match[1]) });
    } else if (selection && "id" in selection && /^guest-\d+$/.test(selection.id)) {
      selectedGuestId = selection.id;
      worker.postMessage({ type: "guest-detail-request", guestId: selection.id });
      gameTest.ready = false;
    }
  } else if (action.type === "construction-delete-mode") {
    deleteMode = action.enabled;
  } else if (action.type.startsWith("construction-")) {
    worker.postMessage(action);
  } else if (action.type === "pan" || action.type === "zoom" || action.type === "rotate") {
    camera = applyCameraAction(camera, viewport, action);
  }
}
bindInput(gameCanvas, dispatch);

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
worker.addEventListener("message", (event: MessageEvent<CoasterWorldMessage | ConstructionState | ({ type: "snapshot"; tickCount: number } & CoasterSnapshot) | { type: "ready" } | { type: "guest-detail"; guestId: string; detail: GuestDetail | null }>) => {
  const message = event.data;
  if (message.type === "world") {
    const coasterGeometry = message.coasterGeometry ?? world?.coasterGeometry;
    const coasterRails = message.coasterRails ?? world?.coasterRails;
    const coasterTies = message.coasterTies ?? world?.coasterTies;
    if (!coasterGeometry || !coasterRails || !coasterTies) throw new Error("Initial coaster geometry is missing");
    world = { ...message, coasterGeometry, coasterRails, coasterTies };
    gameTest.ready = false;
  } else if (message.type === "construction-state") {
    construction = message;
    panel.update(message);
    gameTest.ready = false;
  } else if (message.type === "guest-detail") {
    if (message.guestId === selectedGuestId && message.detail) guestPanel.update(message.detail);
  } else if (message.type === "snapshot") {
    previous = current;
    if (!frozen || !current || current.guestIds.length === 0) current = message;
    receivedAt = performance.now();
    output.value = String(message.tickCount);
    gameCanvas.dataset.agentCount = String(current?.guestIds.length ?? 0);
  }
});

function frame(now: number): void {
  if (!world) return;
  if (frozen && gameTest.ready) return;
  const interpolation = frozen ? 1 : Math.max(0, Math.min(1, (now - receivedAt) / (1000 / 30)));
  const people = current ? buildPeople(previous, current, interpolation, camera.rotation, atlas) : [];
  renderedGuests = people;
  const selectedGuest = people.find((guest) => guest.id === selectedGuestId);
  if (selectedGuest) guestPanel.setAnchor(projectWorld(camera, viewport, { x: selectedGuest.x, y: selectedGuest.y, z: selectedGuest.height / 4 }));
  const trainPoses = current ? interpolateTrainPoses(previous?.trainPoses ?? current.trainPoses, current.trainPoses, interpolation) : new Float32Array();
  const constructionTrainPoses = current?.constructionTrainPoses ?? noGeometry;
  const sprites = people;
  const view = { ...world, sprites, trainPoses };
  const currentWorld = world;
  renderer.render(view, camera, frozen ? 0 : Math.floor(now / 250) % 4, [
    (renderCamera, renderViewport, renderAtlas) => buildSelectionRing(selectedGuest, renderCamera, renderViewport, renderAtlas),
    (renderCamera, renderViewport, renderAtlas, scale) => [
      ...staticTrack(currentWorld.coasterRails, currentWorld.coasterTies, currentWorld.coasterSupports, renderCamera, renderViewport),
      ...(construction ? builtTrack(construction.placedRails, construction.placedTies, construction.placedSupports, renderCamera, renderViewport).map((quad) => ({ ...quad, id: `construction-${quad.id}` })) : []),
      ...buildGhostQuads(stationGhost(), noGeometry, noGeometry, true, renderCamera, renderViewport, trackStyle(renderAtlas)).map((quad) => ({ ...quad, color: [0.25, 0.55, 0.85, 0.55] as const })),
      ...(construction ? buildGhostQuads(construction.previewRails, construction.previewTies, construction.previewSupports, construction.canAppend, renderCamera, renderViewport, trackStyle(renderAtlas)) : []),
      ...buildTrackQuads(noGeometry, noGeometry, noGeometry, trainPoses, renderCamera, renderViewport, trackStyle(renderAtlas)),
      ...buildTrackQuads(noGeometry, noGeometry, noGeometry, constructionTrainPoses, renderCamera, renderViewport, trackStyle(renderAtlas)),
      ...buildRiderQuads(trainPoses, renderCamera, renderViewport, renderAtlas, scale),
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
