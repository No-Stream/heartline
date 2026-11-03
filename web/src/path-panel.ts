export type PathKind = "normal" | "queue";
export interface PathSelection { kind: PathKind; style: number }

export function createPathPanel(
  container: HTMLElement,
  build: (selection: PathSelection) => void,
  remove: () => void,
  cancel: () => void,
): { message(text: string): void } {
  container.innerHTML = `<h2>Paths</h2>
    <label>Path type <select aria-label="Path type"><option value="normal">Path</option><option value="queue">Queue</option></select></label>
    <label>Path style <select aria-label="Path style"><option value="0">Classic</option><option value="1">Cool</option><option value="2">Warm</option><option value="3">Garden</option></select></label>
    <button type="button" data-build>Build paths</button><button type="button" data-remove>Remove paths</button><button type="button" data-cancel>Cancel</button>
    <p>Drag between tiles to build a path. Choose a path to remove it.</p><p role="status"></p>`;
  const kind = container.querySelector<HTMLSelectElement>('[aria-label="Path type"]')!;
  const style = container.querySelector<HTMLSelectElement>('[aria-label="Path style"]')!;
  const status = container.querySelector<HTMLElement>('[role="status"]')!;
  container.querySelector("[data-build]")!.addEventListener("click", () => {
    build({ kind: kind.value as PathKind, style: Number(style.value) });
    status.textContent = "Drag between tiles to build a path.";
  });
  container.querySelector("[data-remove]")!.addEventListener("click", () => { remove(); status.textContent = "Choose a path to remove."; });
  container.querySelector("[data-cancel]")!.addEventListener("click", () => { cancel(); status.textContent = "Path selection cancelled."; });
  return { message(text) { status.textContent = text; } };
}
