/** Player-controlled pause state, independent of the worker's test freeze. */
export function createGamePauseState(initiallyPaused = false): {
  isPaused(): boolean;
  toggle(): boolean;
} {
  let paused = initiallyPaused;
  return {
    isPaused: () => paused,
    toggle: () => {
      paused = !paused;
      return paused;
    },
  };
}
