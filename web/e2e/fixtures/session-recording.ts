import type { WorkerRecording } from "../../src/session-recording";

declare global {
  interface Window { recordWorkerSession(): Promise<WorkerRecording> }
}

window.recordWorkerSession = () => new Promise((resolve, reject) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  worker.onerror = (event) => { worker.terminate(); reject(new Error(event.message)); };
  worker.onmessage = (event: MessageEvent<{ type: string; recording: WorkerRecording }>) => {
    if (event.data.type === "ready") {
      worker.postMessage({ type: "set-surface", x: 10, y: 20, corners: new Int16Array([1, 2, 3, 4]), water: -1 });
      worker.postMessage({ type: "construction-start", x: 20, y: 20, z: 8, heading: 0 });
      worker.postMessage({ type: "test-advance", count: 601 });
    } else if (event.data.type === "test-advanced") {
      setTimeout(() => worker.postMessage({ type: "export-session", requestId: 1 }), 200);
    } else if (event.data.type === "session-export") {
      worker.terminate();
      resolve(event.data.recording);
    }
  };
  worker.postMessage({ type: "init", seed: 2026n, park: "new", guestCount: 0, frozen: true, manualBlockAnalysis: false });
});
