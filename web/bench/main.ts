import { createCustom } from "./custom";
import { createPixi } from "./pixi";
import { createAtlas, markers, Scene } from "./scene";
import { summarizeTimings } from "./statistics";

type Backend = "webgpu" | "webgl2";
type TimingMode = "raf" | "work";
type Result = {
  timing: TimingMode;
  meanMs: number;
  p95Ms: number;
  frames: number;
  durationMs: number;
  backend: Backend;
  adapter: string;
  cpuMeanMs?: number;
  cpuP95Ms?: number;
};
type Benchmark = { ready: boolean; error?: string; result?: Result; markers: typeof markers };
type Renderer = {
  adapter: string;
  render(data: Float32Array, count: number): void;
  waitForGpu(): void | Promise<void>;
};
declare global { interface Window { benchmark: Benchmark } }

window.benchmark = { ready: false, markers };
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const canvas = document.querySelector<HTMLCanvasElement>("#scene")!;
const settings = document.querySelector<HTMLFormElement>("#settings")!;
const params = new URLSearchParams(location.search);
const rendererName = params.get("renderer") ?? "custom";
const backendName = params.get("backend") ?? "auto";
const timingName = params.get("timing") ?? "raf";
const rendererSelect = settings.elements.namedItem("renderer") as HTMLSelectElement;
const backendSelect = settings.elements.namedItem("backend") as HTMLSelectElement;
rendererSelect.value = rendererName;
backendSelect.value = backendName;
settings.addEventListener("submit", (event) => {
  event.preventDefault();
  const query = new URLSearchParams({ renderer: rendererSelect.value, backend: backendSelect.value });
  if (timingName === "work") query.set("timing", "work");
  location.search = query.toString();
});

async function start(): Promise<void> {
  if (rendererName !== "custom" && rendererName !== "pixi") throw new Error(`Unknown renderer: ${rendererName}`);
  if (backendName !== "auto" && backendName !== "webgpu" && backendName !== "webgl2") throw new Error(`Unknown backend: ${backendName}`);
  if (timingName !== "raf" && timingName !== "work") throw new Error(`Unknown timing mode: ${timingName}`);
  const gpu = navigator.gpu;
  const adapter = gpu && backendName !== "webgl2" ? await gpu.requestAdapter() : null;
  const backend: Backend = backendName === "auto" ? (adapter ? "webgpu" : "webgl2") : backendName;
  if (backend === "webgpu" && !adapter) throw new Error("WebGPU adapter is unavailable");
  const createRenderer = rendererName === "custom" ? createCustom : createPixi;
  const renderer: Renderer = await createRenderer(canvas, backend, createAtlas());
  const adapterName = renderer.adapter;
  const scene = new Scene();
  const requestedDuration = Number(params.get("duration") ?? 10000);
  if (!Number.isFinite(requestedDuration)) throw new Error("Invalid benchmark duration");
  const duration = Math.max(10000, requestedDuration);
  window.benchmark.ready = true;
  status.textContent = `Running ${rendererName} with ${backend} (${adapterName}), ${timingName} timing…`;

  if (timingName === "work") {
    runWorkTiming(renderer, scene, backend, adapterName, duration);
  } else {
    runRafTiming(renderer, scene, backend, adapterName, duration);
  }
}

function runRafTiming(renderer: Renderer, scene: Scene, backend: Backend, adapter: string, duration: number): void {
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
      if (elapsed >= duration) {
        const summary = summarizeTimings(frameTimes);
        window.benchmark.result = {
          timing: "raf", ...summary, frames: frameTimes.length, durationMs: elapsed, backend, adapter,
        };
        status.textContent = `${rendererName} / ${backend} rAF: mean ${summary.meanMs.toFixed(2)} ms, p95 ${summary.p95Ms.toFixed(2)} ms (${frameTimes.length} frames)`;
        return;
      }
      requestAnimationFrame(frame);
    } catch (error: unknown) {
      reportError(error);
    }
  }
  requestAnimationFrame(frame);
}

function runWorkTiming(renderer: Renderer, scene: Scene, backend: Backend, adapter: string, duration: number): void {
  const warmupFrames = 60;
  const minimumMeasuredFrames = 300;
  const totalTimes: number[] = [];
  const cpuTimes: number[] = [];
  const channel = new MessageChannel();
  let frameNumber = 0;
  let warmupStart = 0;
  let measurementStart = 0;

  channel.port1.onmessage = async (): Promise<void> => {
    try {
      const frameStart = performance.now();
      if (!warmupStart) warmupStart = frameStart;
      scene.update(frameStart - warmupStart);
      renderer.render(scene.data, scene.count);
      const cpuElapsed = performance.now() - frameStart;
      await renderer.waitForGpu();
      const frameEnd = performance.now();
      frameNumber += 1;

      if (frameNumber === warmupFrames) measurementStart = frameEnd;
      if (frameNumber > warmupFrames) {
        totalTimes.push(frameEnd - frameStart);
        cpuTimes.push(cpuElapsed);
        const measuredDuration = frameEnd - measurementStart;
        if (totalTimes.length >= minimumMeasuredFrames || measuredDuration >= duration) {
          finishWorkTiming(totalTimes, cpuTimes, measuredDuration, backend, adapter);
          channel.port1.close();
          channel.port2.close();
          return;
        }
      }
      channel.port2.postMessage(null);
    } catch (error: unknown) {
      channel.port1.close();
      channel.port2.close();
      reportError(error);
    }
  };
  channel.port2.postMessage(null);

  function finishWorkTiming(
    totalSamples: number[],
    cpuSamples: number[],
    measuredDuration: number,
    resultBackend: Backend,
    resultAdapter: string,
  ): void {
    const total = summarizeTimings(totalSamples);
    const cpu = summarizeTimings(cpuSamples);
    window.benchmark.result = {
      timing: "work",
      ...total,
      frames: totalSamples.length,
      durationMs: measuredDuration,
      backend: resultBackend,
      adapter: resultAdapter,
      cpuMeanMs: cpu.meanMs,
      cpuP95Ms: cpu.p95Ms,
    };
    status.textContent = `${rendererName} / ${resultBackend} work: total mean ${total.meanMs.toFixed(2)} ms, p95 ${total.p95Ms.toFixed(2)} ms; CPU mean ${cpu.meanMs.toFixed(2)} ms, p95 ${cpu.p95Ms.toFixed(2)} ms (${totalSamples.length} frames)`;
  }
}

function reportError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  window.benchmark.error = message;
  status.textContent = `Benchmark failed: ${message}`;
}

start().catch(reportError);
