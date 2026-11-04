import { describe, expect, it } from "vitest";
import { createGamePauseState } from "./game-pause";

describe("game pause state", () => {
  it("toggles between running and paused", () => {
    const pause = createGamePauseState();
    expect(pause.isPaused()).toBe(false);
    expect(pause.toggle()).toBe(true);
    expect(pause.isPaused()).toBe(true);
    expect(pause.toggle()).toBe(false);
    expect(pause.isPaused()).toBe(false);
  });
});
