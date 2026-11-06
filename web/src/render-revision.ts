export function createRenderRevision() {
  let revision = 0;
  let renderedRevision = -1;
  return {
    invalidate(): void { revision += 1; },
    rendered(available: boolean): void { renderedRevision = available ? revision : -1; },
    ready(): boolean { return renderedRevision === revision; },
  };
}

export function shouldSkipRenderFrame(state: {
  revisionReady: boolean;
  frozen: boolean;
  testMode: boolean;
  livePauseRender: boolean;
  paused: boolean;
  following: boolean;
}): boolean {
  return state.revisionReady && (state.frozen || ((state.testMode || state.livePauseRender) && state.paused && !state.following));
}
