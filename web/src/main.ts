import { drawAgents, type AgentSnapshot } from "./renderer";

const canvas = document.querySelector<HTMLCanvasElement>("canvas");
const tickOutput = document.querySelector<HTMLOutputElement>("[data-testid='tick-count']");
if (!canvas || !tickOutput) throw new Error("Game page elements are missing");
const gameCanvas = canvas;

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
let previous: AgentSnapshot = { positions: new Float32Array(), tickCount: 0 };
let current: AgentSnapshot = previous;
let receivedAt = performance.now();

worker.addEventListener("message", (event: MessageEvent) => {
  if (event.data.type !== "snapshot") return;
  previous = current;
  current = { positions: event.data.positions as Float32Array, tickCount: event.data.tickCount as number };
  receivedAt = performance.now();
  tickOutput.value = String(current.tickCount);
});

function renderFrame(now: number): void {
  const progress = Math.min(1, (now - receivedAt) / (1000 / 30));
  drawAgents(gameCanvas, previous, current, progress);
  requestAnimationFrame(renderFrame);
}

requestAnimationFrame(renderFrame);
