import "./toolbar.css";
import type { Units } from "./units";

const storageKey = "theme-park-sim.units";

function storedUnits(): Units | undefined {
  try {
    const stored = localStorage.getItem(storageKey);
    return stored === "imperial" || stored === "metric" ? stored : undefined;
  } catch (error) {
    if (error instanceof DOMException) return undefined;
    throw error;
  }
}

function storeUnits(units: Units): void {
  try {
    localStorage.setItem(storageKey, units);
  } catch (error) {
    if (!(error instanceof DOMException)) throw error;
  }
}

const unitNames: Record<Units, { short: string; long: string }> = {
  imperial: { short: "ft", long: "feet" },
  metric: { short: "m", long: "metres" },
};

/** A small toggle in the game bar between feet and metres, remembered per viewer where browser storage allows. */
export function createUnitsSetting(container: HTMLElement, changed: (units: Units) => void): { units(): Units } {
  let units: Units = storedUnits() ?? "imperial";
  container.innerHTML = `<button class="theme-toggle units-toggle" type="button" aria-label="Units" data-units></button>`;
  const button = container.querySelector<HTMLButtonElement>("[data-units]");
  if (!button) throw new Error("Units toggle is missing");
  const show = (): void => {
    const other: Units = units === "imperial" ? "metric" : "imperial";
    button.dataset.units = units;
    button.textContent = unitNames[units].short;
    button.title = `Units: ${unitNames[units].long}. Click for ${unitNames[other].long}.`;
  };
  show();
  button.addEventListener("click", () => {
    units = units === "imperial" ? "metric" : "imperial";
    storeUnits(units);
    show();
    changed(units);
  });
  return { units: () => units };
}
