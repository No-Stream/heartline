import { describe, expect, it } from "vitest";
import { createRenderRevision, shouldSkipRenderFrame } from "./render-revision";

describe("render revision", () => {
  it("is ready only after a successful render of the current revision", () => {
    const revision = createRenderRevision();
    expect(revision.ready()).toBe(false);
    revision.rendered(false);
    expect(revision.ready()).toBe(false);
    revision.rendered(true);
    expect(revision.ready()).toBe(true);
    revision.invalidate();
    revision.invalidate();
    expect(revision.ready()).toBe(false);
    revision.rendered(true);
    expect(revision.ready()).toBe(true);
  });

  it("skips a ready paused live frame but renders invalidations, follow motion, and resume", () => {
    const revision = createRenderRevision();
    const livePausedFrame = {
      revisionReady: revision.ready(),
      frozen: false,
      testMode: false,
      livePauseRender: true,
      paused: true,
      following: false,
    };

    expect(shouldSkipRenderFrame(livePausedFrame)).toBe(false);
    revision.rendered(true);
    expect(shouldSkipRenderFrame({ ...livePausedFrame, revisionReady: revision.ready() })).toBe(true);

    revision.invalidate();
    expect(shouldSkipRenderFrame({ ...livePausedFrame, revisionReady: revision.ready() })).toBe(false);
    revision.rendered(true);
    expect(shouldSkipRenderFrame({ ...livePausedFrame, revisionReady: revision.ready(), following: true })).toBe(false);
    expect(shouldSkipRenderFrame({ ...livePausedFrame, revisionReady: revision.ready(), paused: false })).toBe(false);
  });

  it("keeps deterministic frozen test rendering behavior", () => {
    expect(shouldSkipRenderFrame({
      revisionReady: true,
      frozen: true,
      testMode: false,
      livePauseRender: false,
      paused: false,
      following: true,
    })).toBe(true);
  });

  it("skips paused replay frames after the replay unfreezes", () => {
    expect(shouldSkipRenderFrame({
      revisionReady: true,
      frozen: false,
      testMode: true,
      livePauseRender: false,
      paused: true,
      following: false,
    })).toBe(true);
  });

  it("keeps normal live pause rendering unless the explicit flag is enabled", () => {
    expect(shouldSkipRenderFrame({
      revisionReady: true,
      frozen: false,
      testMode: false,
      livePauseRender: false,
      paused: true,
      following: false,
    })).toBe(false);
  });
});
