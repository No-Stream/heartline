import { createCustom } from "./custom";
import { createPixi } from "./pixi";
import { createAtlas, markers, Scene } from "./scene";

type Backend = "webgpu" | "webgl2";
type Result = { meanMs: number; p95Ms: number; frames: number; durationMs: number; backend: Backend; adapter: string };
type Benchmark = { ready: boolean; error?: string; result?: Result; markers: typeof markers };
declare global { interface Window { benchmark: Benchmark } }

window.benchmark = { ready: false, markers };
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const canvas = document.querySelector<HTMLCanvasElement>("#scene")!;
const settings = document.querySelector<HTMLFormElement>("#settings")!;
const params = new URLSearchParams(location.search);
const rendererName = params.get("renderer") ?? "custom";
const backendName = params.get("backend") ?? "auto";
const rendererSelect = settings.elements.namedItem("renderer") as HTMLSelectElement;
const backendSelect = settings.elements.namedItem("backend") as HTMLSelectElement;
rendererSelect.value = rendererName;
backendSelect.value = backendName;
settings.addEventListener("submit", (event) => {
  event.preventDefault();
  const query = new URLSearchParams({ renderer: rendererSelect.value, backend: backendSelect.value });
  location.search = query.toString();
});

async function start(): Promise<void> {
  if (rendererName !== "custom" && rendererName !== "pixi") throw new Error(`Unknown renderer: ${rendererName}`);
  if (backendName !== "auto" && backendName !== "webgpu" && backendName !== "webgl2") throw new Error(`Unknown backend: ${backendName}`);
  const gpu = navigator.gpu;
  const adapter = gpu && backendName !== "webgl2" ? await gpu.requestAdapter() : null;
  const backend: Backend = backendName === "auto" ? (adapter ? "webgpu" : "webgl2") : backendName;
  if (backend === "webgpu" && !adapter) throw new Error("WebGPU adapter is unavailable");
  const createRenderer = rendererName === "custom" ? createCustom : createPixi;
  const renderer = await createRenderer(canvas, backend, createAtlas());
  const adapterName = renderer.adapter;
  const scene = new Scene();
  const requestedDuration = Number(params.get("duration") ?? 10000);
  if (!Number.isFinite(requestedDuration)) throw new Error("Invalid benchmark duration");
  const duration = Math.max(10000, requestedDuration);
  window.benchmark.ready = true;
  status.textContent = `Running ${rendererName} with ${backend} (${adapterName})…`;
  const frameTimes: number[] = [];
  let startTime = 0;
  let previousTime = 0;
  function frame(time: number): void {
    try {
      if (!startTime) { startTime = time; previousTime = time; }
      const elapsed = time - startTime;
      scene.update(elapsed);
      renderer.render(scene.data, scene.count);
      if (elapsed > 0) frameTimes.push(time - previousTime);
      previousTime = time;
      if (elapsed >= duration && !window.benchmark.result) {
        const sorted = [...frameTimes].sort((a, b) => a - b);
        const meanMs = frameTimes.reduce((sum, value) => sum + value, 0) / frameTimes.length;
        const p95Ms = sorted[Math.ceil(sorted.length * 0.95) - 1];
        window.benchmark.result = { meanMs, p95Ms, frames: frameTimes.length, durationMs: elapsed, backend, adapter: adapterName };
        status.textContent = `${rendererName} / ${backend}: mean ${meanMs.toFixed(2)} ms, p95 ${p95Ms.toFixed(2)} ms (${frameTimes.length} frames)`;
      }
      requestAnimationFrame(frame);
    } catch (error: unknown) {
      reportError(error);
    }
  }
  requestAnimationFrame(frame);
}

function reportError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  window.benchmark.error = message;
  status.textContent = `Benchmark failed: ${message}`;
}

start().catch(reportError);
