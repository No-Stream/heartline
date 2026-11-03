import { builderShortcuts, keyHint } from "./input/bindings";
import "./print-modern.css";
import "./toolbar.css";

export type Tool = "none" | "coaster" | "paths" | "landscape" | "facilities" | "scenery";

/** Line drawings set like the piece icons: a flat colour plate off register under a plum key line. */
const icons: Record<Tool, { plate: string; paths: string }> = {
  none: { plate: "var(--slate)", paths: "M2 10c2.5-4 5-6 8-6s5.5 2 8 6c-2.5 4-5 6-8 6s-5.5-2-8-6zM10 7.2a2.8 2.8 0 1 0 0.01 0" },
  coaster: { plate: "var(--coral)", paths: "M1.5 16c3 0 4-11 7.5-11 2.5 0 2.5 5 0 5s-1.5-5 3-5c3 0 3.5 11 6.5 11M5 16V11M14 16V8" },
  paths: { plate: "var(--mustard)", paths: "M2 13l6-3 10 5M4 16l6-3 8 4M2 13l2 3M8 10l2 3M14 4l2 3-4 2" },
  landscape: { plate: "var(--teal)", paths: "M1.5 16l5-8 3 4 3-6 6 10zM6.5 8l1.6 2.6" },
  scenery: { plate: "var(--teal)", paths: "M10 17V9M5 10c-3-2-1-7 2-6 2-3 7-2 7 1 4 0 5 5 1 6-2 2-7 2-10-1z" },
  facilities: { plate: "var(--mustard)", paths: "M3 9h14v7H3zM2 9l2-4h12l2 4M8 16v-4h4v4M5 5v-1.5M15 5v-1.5" },
};
const tools: readonly { tool: Tool; label: string; key: string }[] = [
  { tool: "none", label: "Watch", key: keyHint(builderShortcuts.cancel) },
  { tool: "coaster", label: "Coaster", key: keyHint(builderShortcuts.coasterTool) },
  { tool: "paths", label: "Paths", key: keyHint(builderShortcuts.pathBuild) },
  { tool: "landscape", label: "Landscape", key: keyHint(builderShortcuts.landscapeTool) },
  { tool: "scenery", label: "Scenery", key: keyHint(builderShortcuts.sceneryTool) },
  { tool: "facilities", label: "Facilities", key: keyHint(builderShortcuts.facilitiesTool) },
];
const titles: Record<Tool, string> = {
  none: "Watch the park",
  coaster: "Build a coaster",
  paths: "Build paths",
  landscape: "Shape the land",
  scenery: "Place scenery",
  facilities: "Place facilities and set prices",
};

/** One button per tool, pressed for the open one. Choosing a tool is left to `choose`, which also switches the input mode. */
export function createToolbar(container: HTMLElement, choose: (tool: Tool) => void): { setActive(tool: Tool): void } {
  container.classList.add("toolbar");
  container.setAttribute("role", "toolbar");
  container.setAttribute("aria-label", "Tools");
  container.innerHTML = tools.map(({ tool, label, key }) => `
    <button type="button" class="toolbar__tool" data-tool="${tool}" aria-label="${label}" aria-pressed="false" aria-keyshortcuts="${key === "Esc" ? "Escape" : key}" title="${titles[tool]} (${key})" style="--plate: ${icons[tool].plate}">
      <svg class="toolbar__icon" viewBox="0 0 20 20" width="20" height="20" aria-hidden="true"><path class="toolbar__icon-plate" d="${icons[tool].paths}"/><path class="toolbar__icon-line" d="${icons[tool].paths}"/></svg>
      <span>${label}</span><kbd>${key}</kbd>
    </button>`).join("");
  const buttons = [...container.querySelectorAll<HTMLButtonElement>("[data-tool]")];
  // The open tool's button closes it again, back to watching the park.
  for (const button of buttons) button.addEventListener("click", () => choose(button.getAttribute("aria-pressed") === "true" ? "none" : button.dataset.tool as Tool));
  return {
    setActive(tool) {
      for (const button of buttons) button.setAttribute("aria-pressed", String(button.dataset.tool === tool));
    },
  };
}
