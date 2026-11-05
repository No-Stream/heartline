import type { TrainLayout } from "../../src/train-layout";
import type { WorkerRecording } from "../../src/session-recording";
import type { ReplaySession } from "../../src/session-replay";
import { replaySessionIntoWorker, type ReplayWorkerMessage } from "../../src/session-replay-worker";

declare global {
  interface Window {
    replayCarsSession(session: ReplaySession): Promise<{ recording: WorkerRecording; trainLayout: TrainLayout | null }>;
  }
}

window.replayCarsSession = async (session) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  let trainLayout: TrainLayout | null = null;
  worker.addEventListener("message", (event: MessageEvent<ReplayWorkerMessage>) => {
    if (event.data.type === "construction-state" && event.data.complete === true) {
      trainLayout = event.data.trainLayout as TrainLayout | null;
    }
  });
  try {
    const ready = new Promise<void>((resolve) => worker.addEventListener("message", (event: MessageEvent<ReplayWorkerMessage>) => {
      if (event.data.type === "ready") resolve();
    }));
    worker.postMessage({ type: "init", seed: BigInt(session.seed), park: session.park, guestCount: session.guestCount, frozen: true, manualBlockAnalysis: true });
    await ready;
    const exported = await replaySessionIntoWorker(worker, session, { action: { type: "export-session", requestId: 1 }, replyType: "session-export" });
    return { recording: exported.recording as WorkerRecording, trainLayout };
  } finally {
    worker.terminate();
  }
};
