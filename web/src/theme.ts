import "./print-modern.css";
import "./toolbar.css";

export type Theme = "light" | "dark";

const storageKey = "theme-park-sim.theme";
const darkPreference = matchMedia("(prefers-color-scheme: dark)");

/** The viewer's own choice survives a reload; storage can be unavailable, as in a private window, and then the choice lasts for the visit. */
function storedTheme(): Theme | undefined {
  try {
    const stored = localStorage.getItem(storageKey);
    return stored === "light" || stored === "dark" ? stored : undefined;
  } catch (error) {
    if (error instanceof DOMException) return undefined;
    throw error;
  }
}

function storeTheme(theme: Theme): void {
  try {
    localStorage.setItem(storageKey, theme);
  } catch (error) {
    if (!(error instanceof DOMException)) throw error;
  }
}

/**
 * A printed tab in the game bar that switches the interface between paper and deep ink. It follows the system
 * preference until the viewer chooses, and sets `data-theme` on the document so the stylesheet's tokens follow it.
 * `changed` runs on every later switch.
 */
export function createThemeToggle(container: HTMLElement, changed: (theme: Theme) => void): { theme(): Theme } {
  let chosen = storedTheme();
  let theme: Theme = chosen ?? (darkPreference.matches ? "dark" : "light");
  container.innerHTML = `
    <button type="button" class="theme-toggle" aria-label="Dark interface" title="Dark interface" style="--plate: var(--slate)">
      <svg class="toolbar__icon" viewBox="0 0 20 20" width="20" height="20" aria-hidden="true"><path class="toolbar__icon-plate" d="M10 3a7 7 0 1 0 0 14a7 7 0 1 0 0-14z"/><path class="toolbar__icon-line" d="M10 3a7 7 0 1 0 0 14a7 7 0 1 0 0-14zM10 3v14"/><path class="theme-toggle__ink" d="M10 3a7 7 0 0 0 0 14z"/></svg>
    </button>`;
  const button = container.querySelector<HTMLButtonElement>("button")!;
  function apply(next: Theme): void {
    theme = next;
    document.documentElement.dataset.theme = theme;
    button.setAttribute("aria-pressed", String(theme === "dark"));
  }
  button.addEventListener("click", () => {
    chosen = theme === "dark" ? "light" : "dark";
    storeTheme(chosen);
    apply(chosen);
    changed(theme);
  });
  darkPreference.addEventListener("change", () => {
    if (chosen) return;
    apply(darkPreference.matches ? "dark" : "light");
    changed(theme);
  });
  apply(theme);
  return { theme: () => theme };
}
