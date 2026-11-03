import { applyCameraAction, projectWorld, type Camera } from "./camera";
import { guestViewForRotation } from "./guest-view";
import { bindInput } from "./input/bindings";
import type { InputAction } from "./input/actions";
import type { GameTest } from "./game-test";
import { createWorldRenderer, loadAtlas, terrainHeightAt, type WorldView, type WorldSprite } from "./render/index";
import type { WorldSnapshot } from "./world-snapshot";

const canvas = document.querySelector<HTMLCanvasElement>("canvas");
const tickOutput = document.querySelector<HTMLOutputElement>("[data-testid='tick-count']");
if (!canvas || !tickOutput) throw new Error("Game page elements are missing");
const gameCanvas = canvas;
const output = tickOutput;
const viewport = { width: gameCanvas.width, height: gameCanvas.height };
let camera: Camera = { centerX: 250, centerY: 256, zoom: 1, rotation: 0 };
let world: WorldView | undefined;
let previous: Float32Array = new Float32Array();
let current: Float32Array = new Float32Array();
let receivedAt = performance.now();
const frozen = new URLSearchParams(location.search).has("test");
const timing = new URLSearchParams(location.search).get("timing") === "work";
const { atlas, images } = await loadAtlas(new URL("./atlas.json", document.baseURI).href);
const renderer = createWorldRenderer(gameCanvas, atlas, images);

const gameTest: GameTest = {
  ready: false,
  setCamera(value) { camera = value; gameTest.ready = false; },
  project(point) { return projectWorld(camera, viewport, point); },
  pick(x, y) { return renderer.pick(x, y); },
};
window.gameTest = gameTest;

function dispatch(action: InputAction): void {
  if (action.type === "select") {
    const selection = renderer.pick(action.position.x, action.position.y);
    gameCanvas.dataset.selection = JSON.stringify(selection);
  } else camera = applyCameraAction(camera, viewport, action);
}
bindInput(gameCanvas, dispatch);

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
worker.addEventListener("message", (event: MessageEvent<WorldSnapshot | { type: "snapshot"; positions: Float32Array; tickCount: number } | { type: "ready" }>) => {
  const message = event.data;
  if (message.type === "world") {
    world = message;
  } else if (message.type === "snapshot") {
    previous = current;
    if (!frozen || current.length === 0) current = message.positions;
    receivedAt = performance.now();
    output.value = String(message.tickCount);
    gameCanvas.dataset.agentCount = String(current.length / 2);
  }
});

function frame(now: number): void {
  if (!world) return;
  const interpolation = frozen ? 1 : Math.min(1, (now - receivedAt) / (1000 / 30));
  const sprites: WorldSprite[] = [];
  for (let i = 0; i < current.length; i += 2) {
    const x = 230 + ((previous[i] ?? current[i]) + (current[i] - (previous[i] ?? current[i])) * interpolation) * 0.3;
    const y = 240 + ((previous[i + 1] ?? current[i + 1]) + (current[i + 1] - (previous[i + 1] ?? current[i + 1])) * interpolation) * 0.3;
    const height = terrainHeightAt(world, x, y);
    sprites.push({ id: `agent-${i / 2}`, sprite: "people/bea", x, y, height, view: guestViewForRotation(camera.rotation), animation: "idle", frame: 0 });
  }
  renderer.render({ ...world, sprites }, camera, frozen ? 0 : Math.floor(now / 250) % 4);
  gameCanvas.dataset.drawnAgents = String(sprites.length);
  gameTest.ready = current.length > 0;
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
