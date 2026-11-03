import "./print-modern.css";
import "./path-panel.css";

export type PathKind = "normal" | "queue";
export interface PathSelection { kind: PathKind; style: number }

export function createPathPanel(
  container: HTMLElement,
  build: (selection: PathSelection) => void,
  remove: () => void,
  cancel: () => void,
): { message(text: string): void; activate(): void } {
  container.classList.add("tool-panel", "path-panel");
  container.innerHTML = `<header class="tool-panel__header">
      <div><p class="tool-panel__eyebrow">Park paths</p><h2>Paths</h2></div>
    </header>
    <div class="tool-panel__section tool-panel__fields">
      <label><span>Path type</span><select aria-label="Path type"><option value="normal">Path</option><option value="queue">Queue</option></select></label>
      <label><span>Path style</span><select aria-label="Path style"><option value="0">Classic</option><option value="1">Cool</option><option value="2">Warm</option><option value="3">Garden</option></select></label>
    </div>
    <div class="tool-panel__section path-panel__actions">
      <button type="button" class="tool-panel__action tool-panel__action--primary" data-build>Build paths</button>
      <button type="button" class="tool-panel__action" data-remove>Remove paths</button>
      <button type="button" class="tool-panel__action tool-panel__action--quiet" data-cancel>Cancel</button>
    </div>
    <p class="tool-panel__hint path-panel__hint">Drag between tiles to build a path. Choose a path to remove it.</p>
    <p class="tool-panel__status path-panel__status" role="status" aria-live="polite"></p>`;
  const kind = container.querySelector<HTMLSelectElement>('[aria-label="Path type"]')!;
  const style = container.querySelector<HTMLSelectElement>('[aria-label="Path style"]')!;
  const status = container.querySelector<HTMLElement>('[role="status"]')!;
  function activate(): void {
    build({ kind: kind.value as PathKind, style: Number(style.value) });
    status.textContent = "Drag between tiles to build a path.";
  }
  container.querySelector("[data-build]")!.addEventListener("click", activate);
  container.querySelector("[data-remove]")!.addEventListener("click", () => { remove(); status.textContent = "Choose a path to remove."; });
  container.querySelector("[data-cancel]")!.addEventListener("click", () => { cancel(); status.textContent = "Path selection cancelled."; });
  return { message(text) { status.textContent = text; }, activate };
}
