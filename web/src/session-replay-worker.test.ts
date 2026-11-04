import { expect, it } from "vitest";
import { replaySessionIntoWorker } from "./session-replay-worker";
import type { ReplaySession } from "./session-replay";

it("waits for complete construction state before replaying the next command", async () => {
  class FakeWorker extends EventTarget {
    readonly sent: string[] = [];
    postMessage(action: { type: string }): void {
      this.sent.push(action.type);
      if (action.type === "construction-start") this.dispatchEvent(new MessageEvent("message", { data: { type: "construction-state", complete: false } }));
      if (action.type === "finish") this.dispatchEvent(new MessageEvent("message", { data: { type: "finished" } }));
    }
  }
  const worker = new FakeWorker();
  const session: ReplaySession = { format: "theme-park-session", version: 1, seed: "1", park: "new", guestCount: 0, commands: [{ tick: 0, command: { type: "construction-start" } }], checkpoints: [], finalTick: 0 };
  const replay = replaySessionIntoWorker(worker as unknown as Worker, session, { action: { type: "finish" }, replyType: "finished" });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(worker.sent).toEqual(["construction-start"]);
  worker.dispatchEvent(new MessageEvent("message", { data: { type: "construction-state", complete: true } }));
  await replay;
  expect(worker.sent).toEqual(["construction-start", "finish"]);
});
