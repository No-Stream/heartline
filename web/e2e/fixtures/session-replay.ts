import type { WorkerRecording } from "../../src/session-recording";
import type { ReplayObservation, ReplaySession } from "../../src/session-replay";
import { replaySessionIntoWorker, type ReplayWorkerMessage } from "../../src/session-replay-worker";

interface ReplayWorkerOutcome {
  readonly recording: WorkerRecording;
  readonly observation: ReplayObservation;
}

declare global {
  interface Window {
    replayWorkerSession(session: ReplaySession): Promise<ReplayWorkerOutcome>;
  }
}

window.replayWorkerSession = async (session) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  let awaitingConstructionCloseResult = false;
  let constructionCanClose: boolean | undefined;
  let constructionPiecesBeforeMutation: number | undefined;
  const observation: {
    constructionPieces?: number;
    chainEnabled?: boolean;
    pathTiles?: number;
    foodStalls?: number;
    constructionClosed?: boolean;
    riders?: number;
    revenue?: string;
  } = {};

  // Registered before the replay's own listener, so each observation is up to date before the replay sends its next command.
  worker.onmessage = (event: MessageEvent<ReplayWorkerMessage>) => {
    const message = event.data;
    if (message.type === "world") {
      const paths = message.paths as Int32Array;
      observation.pathTiles = paths.length / 4;
    } else if (message.type === "construction-state") {
      const track = message.track as unknown[];
      observation.constructionPieces = track.length;
      observation.chainEnabled = message.chain as boolean;
      if (awaitingConstructionCloseResult) {
        observation.constructionClosed = constructionCanClose === true && message.canClose === false;
        awaitingConstructionCloseResult = false;
      }
      if (constructionPiecesBeforeMutation !== undefined) {
        if (track.length !== constructionPiecesBeforeMutation) observation.constructionClosed = false;
        constructionPiecesBeforeMutation = undefined;
      }
      constructionCanClose = message.canClose as boolean;
    } else if (message.type === "snapshot") {
      const riders = message.constructionRiders as Uint32Array;
      observation.riders = riders.length / 3;
    } else if (message.type === "facilities") {
      const facilities = message.facilities as { readonly kind: string }[];
      observation.foodStalls = facilities.filter(({ kind }) => kind === "food_stall").length;
      observation.revenue = message.revenue as string;
    }
  };

  try {
    const ready = new Promise<void>((resolve) => worker.addEventListener("message", (event: MessageEvent<ReplayWorkerMessage>) => {
      if (event.data.type === "ready") resolve();
    }));
    worker.postMessage({ type: "init", seed: BigInt(session.seed), park: session.park, guestCount: session.guestCount, frozen: true, manualBlockAnalysis: true });
    await ready;
    const exported = await replaySessionIntoWorker(worker, session, { action: { type: "export-session", requestId: 1 }, replyType: "session-export" }, (command) => {
      if (command.type === "construction-close") awaitingConstructionCloseResult = true;
      else if (["construction-start", "construction-append", "construction-undo", "construction-delete"].includes(command.type)) {
        constructionPiecesBeforeMutation = observation.constructionPieces ?? 0;
      }
    });
    return {
      recording: exported.recording as WorkerRecording,
      observation: { ...observation },
    };
  } finally {
    worker.terminate();
  }
};
