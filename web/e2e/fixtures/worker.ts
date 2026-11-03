import type { SetSurfaceMessage, WorldSnapshot } from "../../src/world-snapshot";

declare global {
  interface Window {
    checkWorldEdit(): Promise<{ initial: number[]; changed: number[]; worldMessages: number }>;
  }
}

window.checkWorldEdit = () => new Promise((resolve, reject) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  const edit: SetSurfaceMessage = { type: "set-surface", x: 10, y: 20, corners: new Int16Array([1, 2, 3, 4]), water: -1 };
  const offset = (20 * 512 + 10) * 6;
  let initial: number[] = [];
  let changed: number[] = [];
  let worldMessages = 0;
  worker.onerror = (event) => { worker.terminate(); reject(new Error(event.message)); };
  worker.onmessage = (event: MessageEvent<WorldSnapshot | { type: "snapshot" } | { type: "ready" } | { type: "surface-edited"; changed: boolean }>) => {
    if (event.data.type === "world") {
      worldMessages++;
      const surface = [...event.data.surfaces.slice(offset, offset + 6)];
      if (worldMessages === 1) { initial = surface; worker.postMessage(edit); }
      else if (worldMessages === 2) changed = surface;
    } else if (event.data.type === "surface-edited") {
      if (event.data.changed) worker.postMessage(edit);
      else {
        worker.terminate();
        resolve({ initial, changed, worldMessages });
      }
    }
  };
});
