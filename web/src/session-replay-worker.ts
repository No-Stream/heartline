import { sendWorkerAction, type WorkerAction } from "./protocol";
import { prepareReplayCommand, type ReplaySession } from "./session-replay";

export interface ReplayWorkerMessage {
  readonly type: string;
  readonly [key: string]: unknown;
}

interface WorkerMessageWaiter {
  readonly resolve: (message: ReplayWorkerMessage) => void;
  readonly reject: (error: Error) => void;
  readonly timeout: ReturnType<typeof setTimeout>;
}

/** Commands whose effect the replay must see before it goes on, because the worker answers them with a construction state. */
const constructionCommandsAwaitingState = new Set(["construction-start", "construction-append", "construction-undo", "construction-delete", "construction-close", "construction-finish-preview", "construction-finish-confirm", "construction-finish-cancel", "construction-set-cars-per-train"]);
const replyTimeoutMs = 30_000;

/**
 * Drives an initialized, frozen worker with manual block analysis through a recorded session, from tick zero to the
 * session's final tick, then sends `finalAction` (export the recording, or resume live play) and returns its reply.
 *
 * Listeners the caller registered on the worker before this call see each message before the replay continues, so
 * `beforeCommand` can rely on observations of every earlier reply.
 */
export async function replaySessionIntoWorker(
  worker: Worker,
  session: ReplaySession,
  finalAction: { readonly action: Record<string, unknown>; readonly replyType: string },
  beforeCommand: (command: Record<string, unknown> & { readonly type: string }) => void = () => {},
): Promise<ReplayWorkerMessage> {
  const waiters = new Map<string, WorkerMessageWaiter[]>();
  const rejectWaiters = (error: Error): void => {
    for (const pending of waiters.values()) {
      for (const waiter of pending) {
        clearTimeout(waiter.timeout);
        waiter.reject(error);
      }
    }
    waiters.clear();
  };
  const onMessage = (event: MessageEvent<ReplayWorkerMessage>): void => {
    if (event.data.type === "construction-state" && event.data.complete === false) return;
    const pending = waiters.get(event.data.type);
    if (!pending) return;
    waiters.delete(event.data.type);
    for (const waiter of pending) {
      clearTimeout(waiter.timeout);
      waiter.resolve(event.data);
    }
  };
  let workerError: Error | undefined;
  const failReplay = (error: Error): void => {
    workerError ??= error;
    rejectWaiters(error);
  };
  const onError = (event: ErrorEvent): void => failReplay(new Error(`Replay worker error: ${event.message}`));
  const onMessageError = (): void => failReplay(new Error("Replay worker could not deserialize a message"));
  const postAndWait = (action: Record<string, unknown>, replyType: string): Promise<ReplayWorkerMessage> => {
    if (workerError) return Promise.reject(workerError);
    const reply = new Promise<ReplayWorkerMessage>((resolve, reject) => {
      const timeout = setTimeout(() => {
        waiters.set(replyType, (waiters.get(replyType) ?? []).filter((waiter) => waiter.timeout !== timeout));
        reject(new Error(`Timed out waiting for worker message ${replyType}`));
      }, replyTimeoutMs);
      waiters.set(replyType, [...(waiters.get(replyType) ?? []), { resolve, reject, timeout }]);
    });
    sendWorkerAction(worker, action as WorkerAction);
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

  worker.addEventListener("message", onMessage);
  worker.addEventListener("error", onError);
  worker.addEventListener("messageerror", onMessageError);
  try {
    let tick = 0;
    for (const { tick: commandTick, command } of session.commands) {
      tick = await advanceTo(tick, commandTick);
      const preparedCommand = prepareReplayCommand(command);
      beforeCommand(command);
      if (command.type === "construction-finish-preview") await postAndWait({ type: "construction-finish-await" }, "construction-finish-ready");
      if (command.type === "test-advance-block-analysis" || command.type === "test-advance-block-placement") await postAndWait(preparedCommand, "test-block-analysis-advanced");
      else if (constructionCommandsAwaitingState.has(command.type)) await postAndWait(preparedCommand, "construction-state");
      else sendWorkerAction(worker, preparedCommand as WorkerAction);
    }
    await advanceTo(tick, session.finalTick);
    const finalReply = await postAndWait(finalAction.action, finalAction.replyType);
    if (workerError) throw workerError;
    return finalReply;
  } finally {
    worker.removeEventListener("message", onMessage);
    worker.removeEventListener("error", onError);
    worker.removeEventListener("messageerror", onMessageError);
    rejectWaiters(new Error("Session replay ended before a worker reply arrived"));
  }
}
