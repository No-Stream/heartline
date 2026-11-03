import "./landscaping-panel.css";
import type { InputAction } from "./input/actions";
import { LANDSCAPING_TOOLS, type LandscapingSelection, type LandscapingTool } from "./landscaping-tool";

const TOOL_LABELS: Record<LandscapingTool, string> = {
  raise_corner: "Raise corner",
  lower_corner: "Lower corner",
  raise_tiles: "Raise tiles",
  lower_tiles: "Lower tiles",
  flatten: "Flatten",
  smooth: "Smooth",
  raise_water: "Raise water",
  lower_water: "Lower water",
  carve: "Carve space",
  fill: "Fill space",
};

function readInteger(input: HTMLInputElement | HTMLSelectElement, label: string): number {
  const value = Number(input.value);
  if (!Number.isInteger(value)) throw new RangeError(`${label} must be a whole number.`);
  return value;
}

export function createLandscapingPanel(
  container: HTMLElement,
  dispatch: (action: InputAction) => void,
): {
  message(text: string): void;
  setUndergroundView(enabled: boolean, level?: number): void;
} {
  container.classList.add("landscaping");
  const toolButtons = LANDSCAPING_TOOLS.map((tool) => `<button type="button" data-landscape-tool="${tool}" aria-pressed="${tool === "raise_tiles"}">${TOOL_LABELS[tool]}</button>`).join("");
  const brushOptions = Array.from({ length: 9 }, (_, index) => {
    const size = index + 1;
    return `<option value="${size}"${size === 3 ? " selected" : ""}>${size} × ${size}</option>`;
  }).join("");
  container.innerHTML = `<header class="landscaping__header">
      <div><p class="landscaping__eyebrow">Shape the park</p><h2>Landscaping</h2></div>
    </header>
    <div class="landscaping__tools" role="group" aria-label="Landscaping tools">${toolButtons}</div>
    <div class="landscaping__fields">
      <label><span>Brush size</span><select data-landscape-brush aria-label="Brush size">${brushOptions}</select></label>
      <label><span>Flatten height</span><input data-landscape-height aria-label="Flatten height" type="number" min="-32768" max="32767" step="1" value="8"></label>
      <label><span>Carve bottom</span><input data-landscape-bottom aria-label="Carve bottom" type="number" min="-32768" max="32767" step="1" value="2"></label>
      <label><span>Carve ceiling</span><input data-landscape-top aria-label="Carve ceiling" type="number" min="-32768" max="32767" step="1" value="6"></label>
    </div>
    <div class="landscaping__actions">
      <button type="button" class="landscaping__action landscaping__action--primary" data-landscape-apply>Apply tool</button>
      <button type="button" class="landscaping__action landscaping__action--quiet" data-landscape-cancel>Cancel tool</button>
    </div>
    <div class="landscaping__underground">
      <div class="landscaping__underground-heading"><h3>Underground view</h3><button type="button" data-underground-toggle aria-pressed="false">Show underground</button></div>
      <label><span>Cut level</span><input data-underground-level aria-label="Underground cut level" type="range" min="-32" max="128" step="1" value="8"><output data-underground-output>8</output></label>
      <p class="landscaping__hint">Cut away ground above this quarter-tile height.</p>
    </div>
    <p class="landscaping__hint">Choose a tool, then drag over the park. Corner tools use the picked corner.</p>
    <p class="landscaping__status" role="status" aria-live="polite"></p>`;

  const status = container.querySelector<HTMLElement>("[role='status']")!;
  const brush = container.querySelector<HTMLSelectElement>("[data-landscape-brush]")!;
  const height = container.querySelector<HTMLInputElement>("[data-landscape-height]")!;
  const bottom = container.querySelector<HTMLInputElement>("[data-landscape-bottom]")!;
  const top = container.querySelector<HTMLInputElement>("[data-landscape-top]")!;
  const level = container.querySelector<HTMLInputElement>("[data-underground-level]")!;
  const levelOutput = container.querySelector<HTMLOutputElement>("[data-underground-output]")!;
  const undergroundButton = container.querySelector<HTMLButtonElement>("[data-underground-toggle]")!;
  let selectedTool: LandscapingTool = "raise_tiles";
  let undergroundEnabled = false;

  for (const button of container.querySelectorAll<HTMLButtonElement>("[data-landscape-tool]")) {
    button.addEventListener("click", () => {
      const tool = button.dataset.landscapeTool as LandscapingTool | undefined;
      if (!tool || !LANDSCAPING_TOOLS.includes(tool)) throw new Error("Unknown landscaping tool");
      selectedTool = tool;
      for (const candidate of container.querySelectorAll<HTMLButtonElement>("[data-landscape-tool]")) {
        candidate.setAttribute("aria-pressed", String(candidate === button));
      }
      status.textContent = `${TOOL_LABELS[tool]} selected.`;
    });
  }

  container.querySelector<HTMLButtonElement>("[data-landscape-apply]")!.addEventListener("click", () => {
    try {
      const selection: LandscapingSelection = {
        tool: selectedTool,
        size: readInteger(brush, "Brush size"),
        height: readInteger(height, "Flatten height"),
        bottom: readInteger(bottom, "Carve bottom"),
        top: readInteger(top, "Carve ceiling"),
      };
      if (selection.size < 1 || selection.size > 9 || selection.bottom >= selection.top) throw new RangeError("Choose a brush size from 1 to 9 and a ceiling above the carve floor.");
      dispatch({ type: "landscaping-activate", selection });
      status.textContent = `${TOOL_LABELS[selectedTool]} active. Drag over the park to apply it.`;
    } catch (error) {
      if (!(error instanceof RangeError)) throw error;
      status.textContent = error.message;
    }
  });

  container.querySelector<HTMLButtonElement>("[data-landscape-cancel]")!.addEventListener("click", () => {
    dispatch({ type: "landscaping-cancel" });
    status.textContent = "Landscaping tool cancelled.";
  });

  level.addEventListener("input", () => {
    const nextLevel = readInteger(level, "Underground cut level");
    levelOutput.value = String(nextLevel);
    levelOutput.textContent = String(nextLevel);
    dispatch({ type: "underground-view-level", level: nextLevel });
  });

  undergroundButton.addEventListener("click", () => {
    undergroundEnabled = !undergroundEnabled;
    undergroundButton.textContent = undergroundEnabled ? "Show surface" : "Show underground";
    undergroundButton.setAttribute("aria-pressed", String(undergroundEnabled));
    dispatch({ type: "underground-view-toggle" });
  });

  return {
    message(text: string): void { status.textContent = text; },
    setUndergroundView(enabled: boolean, nextLevel?: number): void {
      undergroundEnabled = enabled;
      undergroundButton.textContent = undergroundEnabled ? "Show surface" : "Show underground";
      undergroundButton.setAttribute("aria-pressed", String(undergroundEnabled));
      if (nextLevel !== undefined) {
        level.value = String(nextLevel);
        levelOutput.value = String(nextLevel);
        levelOutput.textContent = String(nextLevel);
      }
    },
  };
}
