import type { WorkerRecording } from "../../src/session-recording";
import { prepareReplayCommand, type ReplayObservation, type ReplaySession } from "../../src/session-replay";

interface ReplayWorkerMessage {
  readonly type: string;
  readonly [key: string]: unknown;
}

interface ReplayWorkerOutcome {
  readonly recording: WorkerRecording;
  readonly observation: ReplayObservation;
}

interface WorkerMessageWaiter {
  readonly resolve: (message: ReplayWorkerMessage) => void;
  readonly reject: (error: Error) => void;
  readonly timeout: ReturnType<typeof setTimeout>;
}

declare global {
  interface Window {
    replayWorkerSession(session: ReplaySession): Promise<ReplayWorkerOutcome>;
  }
}

window.replayWorkerSession = async (session) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  const waiters = new Map<string, WorkerMessageWaiter[]>();
  let workerError: Error | undefined;
  let awaitingConstructionCloseResult = false;
  let constructionCanClose: boolean | undefined;
  let constructionPiecesBeforeMutation: number | undefined;
  const initialObservation: {
    constructionPieces?: number;
    chainEnabled?: boolean;
    pathTiles?: number;
    foodStalls?: number;
    constructionClosed?: boolean;
    riders?: number;
    revenue?: string;
  } = {};
  const observation = initialObservation;

  const resolveWaiters = (type: string, message: ReplayWorkerMessage): void => {
    const pending = waiters.get(type);
    if (!pending) return;
    waiters.delete(type);
    for (const waiter of pending) {
      clearTimeout(waiter.timeout);
      waiter.resolve(message);
    }
  };
  const rejectWaiters = (error: Error): void => {
    for (const pending of waiters.values()) {
      for (const waiter of pending) {
        clearTimeout(waiter.timeout);
        waiter.reject(error);
      }
    }
    waiters.clear();
  };
  const waitFor = (type: string): Promise<ReplayWorkerMessage> => new Promise((resolveMessage, rejectMessage) => {
    const timeout = setTimeout(() => {
      const pending = waiters.get(type) ?? [];
      waiters.set(type, pending.filter((waiter) => waiter.timeout !== timeout));
      rejectMessage(new Error(`Timed out waiting for worker message ${type}`));
    }, 30_000);
    const pending = waiters.get(type) ?? [];
    waiters.set(type, [...pending, { resolve: resolveMessage, reject: rejectMessage, timeout }]);
  });

  worker.onerror = (event) => {
    workerError = new Error(`Replay worker error: ${event.message}`);
    rejectWaiters(workerError);
  };
  worker.onmessageerror = () => {
    workerError = new Error("Replay worker could not deserialize a message");
    rejectWaiters(workerError);
  };
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
    resolveWaiters(message.type, message);
  };

  const postAndWait = async (action: Record<string, unknown>, replyType: string): Promise<ReplayWorkerMessage> => {
    const reply = waitFor(replyType);
    worker.postMessage(action);
    return reply;
  };

  const advanceTo = async (currentTick: number, targetTick: number): Promise<number> => {
    let tick = currentTick;
    while (tick < targetTick) {
      const count = Math.min(targetTick - tick, 100_000);
      await postAndWait({ type: "test-advance", count }, "test-advanced");
      tick += count;
    }
    return tick;
  };

  try {
    const ready = waitFor("ready");
    worker.postMessage({ type: "init", seed: BigInt(session.seed), park: session.park, guestCount: session.guestCount, frozen: true, manualBlockAnalysis: true });
    await ready;

    let tick = 0;
    for (const { tick: commandTick, command } of session.commands) {
      tick = await advanceTo(tick, commandTick);
      const preparedCommand = prepareReplayCommand(command);
      if (preparedCommand.type === "test-advance-block-analysis") {
        await postAndWait(preparedCommand, "test-block-analysis-advanced");
      } else if (preparedCommand.type === "construction-close") {
        const response = waitFor("construction-state");
        awaitingConstructionCloseResult = true;
        worker.postMessage(preparedCommand);
        await response;
      } else if (["construction-start", "construction-append", "construction-undo", "construction-delete"].includes(command.type)) {
        const response = waitFor("construction-state");
        constructionPiecesBeforeMutation = observation.constructionPieces ?? 0;
        worker.postMessage(preparedCommand);
        await response;
      } else {
        worker.postMessage(preparedCommand);
      }
    }
    await advanceTo(tick, session.finalTick);
    const exported = await postAndWait({ type: "export-session", requestId: 1 }, "session-export");
    if (workerError) throw workerError;
    return {
      recording: exported.recording as WorkerRecording,
      observation: { ...observation },
    };
  } finally {
    worker.terminate();
  }
};
