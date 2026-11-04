export function createRenderRevision() {
  let revision = 0;
  let renderedRevision = -1;
  return {
    invalidate(): void { revision += 1; },
    rendered(available: boolean): void { renderedRevision = available ? revision : -1; },
    ready(): boolean { return renderedRevision === revision; },
  };
}
