import { REPORT_SECTION_HTML, renderReportSection, type ReportLocationSelection, type TestingSample } from "../coaster-testing";
import type { ScoreContributionReport } from "../generated/ScoreContributionReport";
import type { InputAction } from "../input/actions";
import { GUEST_PANEL_EDGE as EDGE } from "../guest-panel/placement";
import type { RideInfo } from "./protocol";
import { createRideGraphView } from "./graph-view";
import type { Units } from "../units";
import "./ride-window.css";

export interface RideWindowCallbacks {
  readonly dispatch: (action: InputAction) => void;
  /** Called when the part of the track to highlight changes. */
  readonly onHighlightChange: () => void;
}

export interface RideHighlight {
  readonly samples: readonly TestingSample[];
  readonly distance: number;
}

export interface RideWindow {
  setUnits(units: Units): void;
  update(info: RideInfo, testMessage: string): void;
  /** The point of the track the pointer is on in the graph, or the spot picked from the reasons list. */
  highlight(): RideHighlight | undefined;
  close(): void;
}


function nearestSampleToDistance(samples: readonly TestingSample[], distance: number): number {
  let nearest = 0;
  for (let index = 1; index < samples.length; index++) {
    if (Math.abs(samples[index].distance - distance) < Math.abs(samples[nearest].distance - distance)) nearest = index;
  }
  return nearest;
}

/** A dialog beside the park view with a coaster's latest test report: scores, g-force graph, reasons and statistics. */
export function createRideWindow(canvas: HTMLCanvasElement, callbacks: RideWindowCallbacks): RideWindow {
  const root = document.createElement("aside");
  root.className = "ride-window";
  root.dataset.testid = "ride-window";
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-label", "Ride information");
  root.hidden = true;
  root.innerHTML = `
    <header class="guest-panel__header">
      <p class="guest-panel__eyebrow" data-testid="ride-eyebrow"></p>
      <h2 data-testid="ride-name"></h2>
      <button class="guest-panel__close" type="button" data-testid="ride-window-close" aria-label="Close ride information">×</button>
    </header>
    <section class="ride-window__untested" data-testid="ride-untested" hidden>
      <p>This coaster has no current test results. Run a test to see its scores, forces and statistics.</p>
      <button type="button" class="tool-panel__action" data-testid="ride-test">Test coaster</button>
    </section>
    <p class="ride-window__message" data-testid="ride-message" role="status" hidden></p>
    <div class="ride-window__report" data-testid="ride-report" hidden>${REPORT_SECTION_HTML}</div>`;
  document.body.append(root);

  const eyebrow = root.querySelector<HTMLElement>("[data-testid='ride-eyebrow']")!;
  const name = root.querySelector<HTMLElement>("[data-testid='ride-name']")!;
  const untested = root.querySelector<HTMLElement>("[data-testid='ride-untested']")!;
  const message = root.querySelector<HTMLElement>("[data-testid='ride-message']")!;
  const report = root.querySelector<HTMLElement>("[data-testid='ride-report']")!;
  const testButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-test']")!;
  const closeButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-window-close']")!;
  let info: RideInfo | undefined;
  let testMessage = "";
  let units: Units = "imperial";
  let hoveredIndex: number | undefined;
  let pickedLocation: ScoreContributionReport["location"];

  const graphView = createRideGraphView((index) => {
    hoveredIndex = index;
    callbacks.onHighlightChange();
  });
  report.querySelector("[data-rating]")!.after(graphView.element);

  const locations: ReportLocationSelection = {
    selected: () => pickedLocation,
    select(location) {
      pickedLocation = location;
      hoveredIndex = undefined;
      graphView.setCursor(nearestSampleToDistance(info!.report!.samples, location.distance));
      callbacks.onHighlightChange();
    },
  };

  closeButton.addEventListener("click", () => callbacks.dispatch({ type: "ride-close" }));
  testButton.addEventListener("click", () => callbacks.dispatch({ type: "ride-test", ride: info!.ride }));
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !root.hidden) {
      event.preventDefault();
      callbacks.dispatch({ type: "ride-close" });
    }
  });

  function place(): void {
    if (root.hidden) return;
    const bounds = canvas.getBoundingClientRect();
    root.style.left = `${Math.max(bounds.left + EDGE, bounds.right - root.getBoundingClientRect().width - EDGE)}px`;
    root.style.top = `${bounds.top + EDGE}px`;
    root.style.maxHeight = `${Math.max(160, bounds.height - EDGE * 2)}px`;
  }
  new ResizeObserver(place).observe(canvas);
  window.addEventListener("resize", place);

  function render(): void {
    if (!info) return;
    root.dataset.ride = String(info.ride);
    eyebrow.textContent = `${info.ride === 0xffff_ffff ? "Park coaster" : "Your coaster"} · ${info.manufacturer}`;
    name.textContent = info.modelName;
    untested.hidden = info.report !== null;
    report.hidden = info.report === null;
    message.hidden = testMessage === "";
    message.textContent = testMessage;
    renderReportSection(report, info.report, units, locations);
    if (info.report) graphView.update(info.report.samples, info.limits, units);
    root.hidden = false;
    place();
  }

  return {
    setUnits(value) {
      units = value;
      render();
    },
    update(next, nextTestMessage) {
      pickedLocation = undefined;
      hoveredIndex = undefined;
      info = next;
      testMessage = nextTestMessage;
      render();
    },
    highlight() {
      const samples = info?.report?.samples;
      if (!samples) return undefined;
      if (hoveredIndex !== undefined) return { samples, distance: samples[hoveredIndex].distance };
      return pickedLocation ? { samples, distance: pickedLocation.distance } : undefined;
    },
    close() {
      root.hidden = true;
      info = undefined;
      hoveredIndex = undefined;
      pickedLocation = undefined;
    },
  };
}
