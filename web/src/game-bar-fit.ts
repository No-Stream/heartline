export const GAME_BAR_COLLAPSE_STEPS = 8;

/** Return the first collapse level whose measured content fits the available bar width. */
export function selectGameBarFitLevel(availableWidth: number, measureWidth: (level: number) => number): number {
  for (let level = 0; level <= GAME_BAR_COLLAPSE_STEPS; level += 1) {
    if (measureWidth(level) <= availableWidth) return level;
  }
  return GAME_BAR_COLLAPSE_STEPS;
}

/** Keep the game bar fitted as its available width or visible controls change. */
export function fitGameBar(bar: HTMLElement): void {
  let scheduled = false;
  const measureWidth = (level: number): number => {
    bar.dataset.fitLevel = String(level);
    const children = [...bar.children] as HTMLElement[];
    const styles = getComputedStyle(bar);
    const gap = Number.parseFloat(styles.columnGap) || 0;
    const padding = (Number.parseFloat(styles.paddingLeft) || 0) + (Number.parseFloat(styles.paddingRight) || 0);
    const childWidths = children.reduce((sum, child) => sum + (child.hidden ? 0 : child.scrollWidth), 0);
    const visibleChildren = children.filter((child) => !child.hidden).length;
    return childWidths + Math.max(0, visibleChildren - 1) * gap + padding;
  };
  const update = (): void => {
    scheduled = false;
    const availableWidth = bar.clientWidth;
    const level = selectGameBarFitLevel(availableWidth - 1, measureWidth);
    if (bar.dataset.fitLevel !== String(level)) bar.dataset.fitLevel = String(level);
  };
  const schedule = (): void => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(update);
  };
  const observer = new ResizeObserver(schedule);
  observer.observe(bar);
  for (const child of bar.children) observer.observe(child);
  observer.observe(document.documentElement);
  window.addEventListener("resize", schedule);
  const mutationObserver = new MutationObserver((records) => {
    if (records.some((record) => record.type !== "attributes" || record.target !== bar || record.attributeName !== "data-fit-level")) schedule();
  });
  mutationObserver.observe(bar, { childList: true, subtree: true, attributes: true, characterData: true });
  schedule();
}
