import { REPORT_SECTION_HTML, renderReportSection, type ReportLocationSelection, type TestingSample } from "../coaster-testing";
import type { ScoreContributionReport } from "../generated/ScoreContributionReport";
import type { InputAction } from "../input/actions";
import { GUEST_PANEL_EDGE as EDGE } from "../guest-panel/placement";
import type { RideInfo } from "./protocol";
import { createRideGraphView } from "./graph-view";
import type { Units } from "../units";
import "./ride-window.css";
import { rideOpenAction, ridePriceAction, rideRenameAction } from "./controls";

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
    </section>
    <section class="ride-window__controls" data-testid="ride-controls" aria-label="Ride controls" hidden>
      <div class="ride-window__status-row">
        <strong data-testid="ride-state"></strong>
        <button type="button" class="tool-panel__action" data-testid="ride-open-toggle"></button>
      </div>
      <span>Income</span>
      <output data-testid="ride-income" aria-label="Coaster income">0</output>
      <label class="ride-window__price-label" for="ride-window-price">Ticket price</label>
      <div class="stepper stepper--compact ride-window__price-stepper" role="group" aria-label="Coaster ticket price">
        <button type="button" data-price-step="-1" aria-label="Lower the coaster ticket price">−</button>
        <input id="ride-window-price" type="number" min="0" max="65535" step="1" data-testid="ride-price" aria-label="Coaster ticket price">
        <button type="button" data-price-step="1" aria-label="Raise the coaster ticket price">+</button>
      </div>
      <button type="button" class="tool-panel__action" data-testid="ride-rename-toggle">Rename coaster</button>
      <form class="ride-window__rename" data-testid="ride-rename-form" hidden>
        <label for="ride-window-name">Coaster name</label>
        <input id="ride-window-name" type="text" data-testid="ride-name-input" required>
        <button type="submit" class="tool-panel__action" data-testid="ride-rename-submit">Save name</button>
      </form>
      <div class="ride-window__danger" data-testid="ride-demolish-section">
        <button type="button" class="tool-panel__action ride-window__demolish" data-testid="ride-demolish">Demolish coaster</button>
        <div class="ride-window__demolish-confirmation" data-testid="ride-demolish-confirmation" hidden>
          <p>Demolish this coaster? Its track and station will be removed.</p>
          <button type="button" class="tool-panel__action" data-testid="ride-demolish-cancel">Keep coaster</button>
          <button type="button" class="tool-panel__action ride-window__demolish" data-testid="ride-demolish-confirm">Demolish coaster</button>
        </div>
      </div>
    </section>
    <div class="ride-window__actions">
      <button type="button" class="tool-panel__action" data-testid="ride-test">Test coaster</button>
      <button type="button" class="tool-panel__action" data-testid="ride-edit" hidden>Edit coaster</button>
    </div>
    <p class="ride-window__message" data-testid="ride-message" role="status" hidden></p>
    <div class="ride-window__report" data-testid="ride-report" hidden>${REPORT_SECTION_HTML}</div>`;
  document.body.append(root);

  const eyebrow = root.querySelector<HTMLElement>("[data-testid='ride-eyebrow']")!;
  const name = root.querySelector<HTMLElement>("[data-testid='ride-name']")!;
  const untested = root.querySelector<HTMLElement>("[data-testid='ride-untested']")!;
  const message = root.querySelector<HTMLElement>("[data-testid='ride-message']")!;
  const report = root.querySelector<HTMLElement>("[data-testid='ride-report']")!;
  const testButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-test']")!;
  const editButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-edit']")!;
  const closeButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-window-close']")!;
  const rideControls = root.querySelector<HTMLElement>("[data-testid='ride-controls']")!;
  const rideState = root.querySelector<HTMLElement>("[data-testid='ride-state']")!;
  const openButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-open-toggle']")!;
  const income = root.querySelector<HTMLOutputElement>("[data-testid='ride-income']")!;
  const priceInput = root.querySelector<HTMLInputElement>("[data-testid='ride-price']")!;
  const priceButtons = [...root.querySelectorAll<HTMLButtonElement>("[data-price-step]")];
  const renameToggle = root.querySelector<HTMLButtonElement>("[data-testid='ride-rename-toggle']")!;
  const renameForm = root.querySelector<HTMLFormElement>("[data-testid='ride-rename-form']")!;
  const renameInput = root.querySelector<HTMLInputElement>("[data-testid='ride-name-input']")!;
  const demolishButton = root.querySelector<HTMLButtonElement>("[data-testid='ride-demolish']")!;
  const demolishConfirmation = root.querySelector<HTMLElement>("[data-testid='ride-demolish-confirmation']")!;
  const demolishCancel = root.querySelector<HTMLButtonElement>("[data-testid='ride-demolish-cancel']")!;
  const demolishConfirm = root.querySelector<HTMLButtonElement>("[data-testid='ride-demolish-confirm']")!;
  let info: RideInfo | undefined;
  let testMessage = "";
  let units: Units = "imperial";
  let hoveredIndex: number | undefined;
  let pickedLocation: ScoreContributionReport["location"];
  let renameOpen = false;
  let demolishConfirmationOpen = false;

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
  editButton.addEventListener("click", () => callbacks.dispatch({ type: "ride-edit", ride: info!.ride }));
  openButton.addEventListener("click", () => {
    const action = rideOpenAction(info!);
    if (action) callbacks.dispatch(action);
  });
  priceInput.addEventListener("change", () => {
    const action = ridePriceAction(info!.ride, Number(priceInput.value), info!.editable);
    if (priceInput.value !== "" && priceInput.validity.valid && action) callbacks.dispatch(action);
    else priceInput.value = String(info!.price);
  });
  for (const button of priceButtons) {
    button.addEventListener("click", () => {
      const currentPrice = Number(priceInput.value);
      const action = ridePriceAction(info!.ride, currentPrice + Number(button.dataset.priceStep), info!.editable);
      if (action) callbacks.dispatch(action);
    });
  }
  renameToggle.addEventListener("click", () => {
    renameOpen = !renameOpen;
    render();
    if (renameOpen) renameInput.focus();
  });
  renameForm.addEventListener("submit", (event) => {
    event.preventDefault();
    const action = rideRenameAction(info!.ride, info!.name, renameInput.value, info!.editable);
    if (action) {
      callbacks.dispatch(action);
      renameOpen = false;
      render();
    }
  });
  demolishButton.addEventListener("click", () => {
    demolishConfirmationOpen = true;
    render();
  });
  demolishCancel.addEventListener("click", () => {
    demolishConfirmationOpen = false;
    render();
  });
  demolishConfirm.addEventListener("click", () => {
    callbacks.dispatch({ type: "ride-demolish", ride: info!.ride });
  });
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !root.hidden) {
      event.preventDefault();
      callbacks.dispatch({ type: "ride-close" });
    }
  });

  function place(): void {
    if (root.hidden) return;
    const bounds = canvas.getBoundingClientRect();
    root.style.left = `${Math.round(Math.max(bounds.left + EDGE, bounds.right - root.getBoundingClientRect().width - EDGE))}px`;
    root.style.top = `${Math.round(bounds.top + EDGE)}px`;
    root.style.maxHeight = `${Math.max(160, bounds.height - EDGE * 2)}px`;
  }
  new ResizeObserver(place).observe(canvas);
  window.addEventListener("resize", place);

  function render(): void {
    if (!info) return;
    root.dataset.ride = String(info.ride);
    eyebrow.textContent = `${info.modelName} · ${info.manufacturer}`;
    name.textContent = info.name;
    rideControls.hidden = !info.editable;
    rideState.textContent = info.open ? "Open" : "Closed";
    openButton.textContent = info.open ? "Close coaster" : "Open coaster";
    openButton.disabled = !info.complete;
    openButton.title = info.complete ? "" : "Finish the track before opening this coaster.";
    income.textContent = String(info.income);
    if (document.activeElement !== priceInput) priceInput.value = String(info.price);
    priceButtons[0].disabled = info.price === 0;
    priceButtons[1].disabled = info.price === 65535;
    renameToggle.hidden = renameOpen;
    renameForm.hidden = !renameOpen;
    if (document.activeElement !== renameInput) renameInput.value = info.name;
    demolishButton.hidden = demolishConfirmationOpen;
    demolishConfirmation.hidden = !demolishConfirmationOpen;
    editButton.hidden = !info.editable;
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
      if (info?.ride !== next.ride) {
        renameOpen = false;
        demolishConfirmationOpen = false;
      }
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
      renameOpen = false;
      demolishConfirmationOpen = false;
    },
  };
}
