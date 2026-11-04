import { describe, expect, it } from "vitest";
import { createRenderRevision } from "./render-revision";

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
});
