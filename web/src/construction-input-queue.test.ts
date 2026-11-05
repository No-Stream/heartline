import { describe, expect, it } from "vitest";
import { createConstructionInputQueue } from "./construction-input-queue";
import type { InputAction } from "./input/actions";

const append: InputAction = { type: "construction-append" };
const chain: InputAction = { type: "chain-toggle" };

describe("construction input queue", () => {
  it("runs actions in order, waiting for each worker response", () => {
    const sent: InputAction[] = [];
    const queue = createConstructionInputQueue((action) => { sent.push(action); return true; }, () => {});
    queue.submit(append, 1);
    queue.submit(chain, 1);
    queue.submit(append, 1);
    expect(sent).toEqual([append]);
    queue.complete(2, true);
    expect(sent).toEqual([append, chain]);
    queue.complete(2, true);
    expect(sent).toEqual([append, chain, append]);
    queue.complete(3, true);
    expect(queue.pending()).toBe(0);
  });

  it("cancels dependent actions after a refused placement", () => {
    const sent: InputAction[] = [];
    const notices: string[] = [];
    const queue = createConstructionInputQueue((action) => { sent.push(action); return true; }, (notice) => notices.push(notice));
    queue.submit(append, 2);
    queue.submit(chain, 2);
    queue.submit(append, 2);
    queue.complete(2, true);
    expect(sent).toEqual([append]);
    expect(queue.pending()).toBe(0);
    expect(notices.at(-1)).toMatch(/cancelled/i);
  });

  it("bounds waiting inputs and reports when the queue is full", () => {
    const notices: string[] = [];
    const queue = createConstructionInputQueue(() => true, (notice) => notices.push(notice), 2);
    queue.submit(append, 1);
    queue.submit(chain, 1);
    queue.submit(append, 1);
    queue.submit(chain, 1);
    expect(queue.pending()).toBe(2);
    expect(notices.at(-1)).toMatch(/full/i);
  });

  it("cancels waiting input when the editor closes", () => {
    const sent: InputAction[] = [];
    const queue = createConstructionInputQueue((action) => { sent.push(action); return true; }, () => {});
    queue.submit(append, 1);
    queue.submit(chain, 1);
    queue.complete(2, false);
    expect(sent).toEqual([append]);
    expect(queue.pending()).toBe(0);
  });

  it("waits for an externally busy editor and preserves repeated toggles", () => {
    const sent: InputAction[] = [];
    const queue = createConstructionInputQueue((action) => { sent.push(action); return true; }, () => {});
    queue.setBusy(true, 1);
    queue.submit(chain, 1);
    queue.submit(chain, 1);
    queue.submit(append, 1);
    expect(sent).toEqual([]);
    queue.setBusy(false, 1);
    queue.complete(1, true);
    queue.complete(1, true);
    expect(sent).toEqual([chain, chain, append]);
  });

  it("continues when an action cannot produce a worker command", () => {
    const sent: InputAction[] = [];
    const queue = createConstructionInputQueue((action) => { sent.push(action); return action.type !== "piece-group"; }, () => {});
    queue.setBusy(true, 1);
    queue.submit({ type: "piece-group", group: "missing" }, 1);
    queue.submit(append, 1);
    queue.setBusy(false, 1);
    expect(sent).toEqual([{ type: "piece-group", group: "missing" }, append]);
  });

  it("does not apply an old refusal to input after the editor is reopened", () => {
    const sent: InputAction[] = [];
    const queue = createConstructionInputQueue((action) => { sent.push(action); return true; }, () => {});
    queue.submit(append, 1);
    queue.submit(chain, 1);
    queue.cancel();
    queue.submit(append, 1);
    queue.complete(1, true);
    expect(sent).toEqual([append, append]);
  });
});
