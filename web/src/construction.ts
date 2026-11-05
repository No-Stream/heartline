import { COASTER_COLOR_NAMES, COASTER_COLORS, COASTER_PARTS, repaint, type CoasterPart } from "./coaster-colors";
import { constructionEvent } from "./construction-effects";
import { availableBlockBoundaries, blockPanelState, steppedTrainCount } from "./block-panel-state";
import { groupedPieces, pieceDisplayName, pieceInGroup, steppedPiece, stepperOptions, type StepAttribute } from "./construction-choices";
import type { BlockReport, ConstructionElement, ConstructionPiece, ConstructionState, PieceShape } from "./construction-protocol";
import type { InputAction } from "./input/actions";
import { builderShortcuts, groupKeyHint, keyHint } from "./input/bindings";
import { iconViews, pieceIconSvg } from "./piece-icons";
import { displaySpeedToMetresPerSecond, displayToMetres, formatLength, metresPerSecondToDisplay, metresToInput, speedUnitLabel, unitLabel, type Units } from "./units";
import "./print-modern.css";
import "./construction.css";

type PieceAction = Extract<InputAction, { type: "piece-step" | "piece-group" }>;
export interface ConstructionPanel {
  update(state: ConstructionState): void;
  setShapes(shapes: readonly PieceShape[]): void;
  /** Resolve a keyboard piece step against the current choices and select the result. */
  choose(action: PieceAction): void;
  setDeleteMode(enabled: boolean): void;
  setSelectedBlock(index: number | null): void;
  setUnits(units: Units): void;
  setHeightMarkers(track: string, station: string): void;
}

const groupNames: Record<string, string> = { straight: "Straight", turn: "Turns", slope: "Slopes", bank: "Banks", inversion: "Inversions", special: "Special" };
const groupPlates: Record<string, string> = { straight: "var(--slate)", turn: "var(--teal)", slope: "var(--mustard)", bank: "var(--coral)", inversion: "var(--coral)", special: "var(--slate)" };
const blockTrackKindNames: Record<string, string> = { blockBrake: "Brake run", station: "Station", lift: "Lift", launch: "Launch", brake: "Brake", track: "Track" };
const coasterModels = {
  "model:hybrid": {
    id: "hybrid",
    name: "Timber Tumbler",
    manufacturer: "Rocky Road Construction",
    description: "Steel box track on a wooden structure.",
  },
  "model:velocity_viper": {
    id: "velocity_viper",
    name: "Velocity Viper",
    manufacturer: "Impulse Engineering",
    description: "Multi-launch steel track and columns with short, standard, and long launch sections.",
  },
  "model:wooden_twister": {
    id: "wooden_twister",
    name: "Briar Blitz",
    manufacturer: "Great Coaster Workshop",
    description: "Twisting wooden track on a dense wooden structure, with a chain lift and articulated trains.",
  },
} as const;
const coasterPartNames: Record<CoasterPart, string> = { track: "Track", supports: "Supports", trains: "Trains" };
const stationHeight = { minimum: 4, maximum: 100, step: 2, initial: 12 };
const steppers: readonly { attribute: StepAttribute; label: string; previous: string; next: string; keys: readonly [keyof typeof builderShortcuts, keyof typeof builderShortcuts] }[] = [
  { attribute: "turn", label: "Direction", previous: "Turn left", next: "Turn right", keys: ["turnLeft", "turnRight"] },
  { attribute: "slope", label: "Slope", previous: "Slope down", next: "Slope up", keys: ["slopeDown", "slopeUp"] },
  { attribute: "bank", label: "Bank", previous: "Bank left", next: "Bank right", keys: ["bankLeft", "bankRight"] },
];
/** Building comes first; colours and running the ride are settings the player visits when they want them. */
const builderTabs = [
  { id: "build", label: "Build" },
  { id: "colours", label: "Colours" },
  { id: "operate", label: "Operate" },
] as const;
type BuilderTab = (typeof builderTabs)[number]["id"];
const hint = (name: keyof typeof builderShortcuts): string => `<kbd>${keyHint(builderShortcuts[name])}</kbd>`;
function sampledLength(shape: PieceShape): number {
  let length = 0;
  for (let index = 1; index < shape.points.length; index++) {
    const previous = shape.points[index - 1];
    const current = shape.points[index];
    length += Math.hypot(current[0] - previous[0], current[1] - previous[1], current[2] - previous[2]);
  }
  return length;
}

/** Native controls keep every construction action available to keyboard and assistive technology users. */
export function createConstructionPanel(container: HTMLElement, dispatch: (action: InputAction) => void): ConstructionPanel {
  container.classList.add("tool-panel", "builder");
  container.innerHTML = `
    <header class="tool-panel__header"><div><p class="tool-panel__eyebrow">Coaster workshop</p><h2>Build a coaster</h2></div></header>
    <section class="tool-panel__section" aria-label="Coaster model">
      <label class="builder__model"><span class="tool-panel__label">Coaster model</span>
        <select data-model>
          <option value="model:hybrid">${coasterModels["model:hybrid"].name} · ${coasterModels["model:hybrid"].manufacturer}</option>
          <option value="model:velocity_viper">${coasterModels["model:velocity_viper"].name} · ${coasterModels["model:velocity_viper"].manufacturer}</option>
          <option value="model:wooden_twister">${coasterModels["model:wooden_twister"].name} · ${coasterModels["model:wooden_twister"].manufacturer}</option>
        </select>
      </label>
      <p class="tool-panel__hint" data-model-description>${coasterModels["model:hybrid"].description}</p>
    </section>
    <div class="tool-panel__section builder__station">
      <div class="builder__station-height">
        <span class="tool-panel__label" id="station-height-label">Station height</span>
        <div class="stepper stepper--compact" role="group" aria-labelledby="station-height-label">
          <button type="button" data-height-step="-1" aria-label="Lower the station">−</button>
          <input type="number" data-height min="${stationHeight.minimum}" max="${stationHeight.maximum}" step="${stationHeight.step}" value="${stationHeight.initial}" aria-label="Station height above ground (metres)">
          <button type="button" data-height-step="1" aria-label="Raise the station">+</button>
        </div>
        <span class="stepper__unit">m</span>
      </div>
      <button type="button" class="tool-panel__action tool-panel__action--primary" data-start>Place station</button>
      <p class="tool-panel__hint">Choose a tile, then point and click to choose the station direction.</p>
    </div>
    <p role="status" class="tool-panel__status" data-message></p>
    <div data-editor hidden>
      <div class="builder__tabs" role="tablist" aria-label="Coaster builder sections" data-builder-tabs>${builderTabs.map(({ id, label }, index) => `
        <button type="button" role="tab" id="builder-tab-${id}" aria-controls="builder-tabpanel-${id}" aria-selected="${index === 0}" tabindex="${index === 0 ? 0 : -1}" data-builder-tab="${id}">${label}</button>`).join("")}
      </div>
      <div class="builder__tabpanel" role="tabpanel" id="builder-tabpanel-build" aria-labelledby="builder-tab-build" data-builder-tabpanel="build">
        <p class="tool-panel__section builder__ready" data-ready hidden>The track meets the station. Close the circuit with ${hint("close")}</p>
        <section class="tool-panel__section builder__next" aria-label="Chosen piece" data-next>
          <figure class="tool-panel__preview" data-preview><div class="tool-panel__preview-icons" data-preview-icons></div>
            <figcaption><span class="tool-panel__label tool-panel__preview-group" data-preview-group></span><span class="tool-panel__preview-name" data-preview-name></span></figcaption></figure>
          <div class="builder__steppers" data-steppers>${steppers.map(({ attribute, label, previous, next, keys }) => `
            <div class="stepper" role="group" aria-label="Next piece ${attribute}">
              <span class="tool-panel__label">${label}</span>
              <button type="button" data-stepper="${attribute}--1" aria-label="${previous}">−</button>
              <span class="stepper__value" data-stepper-value="${attribute}"></span>
              <button type="button" data-stepper="${attribute}-1" aria-label="${next}">+</button>
              <span class="stepper__keys">${hint(keys[0])}${hint(keys[1])}</span>
            </div>`).join("")}
          </div>
        </section>
        <div class="tool-panel__section builder__chain" data-chain-section>
          <button type="button" class="builder__chain-toggle" data-chain-toggle aria-pressed="false" aria-label="Chain lift" aria-keyshortcuts="${keyHint(builderShortcuts.chain)}" aria-describedby="chain-lift-hint">
            <svg class="builder__chain-links" viewBox="0 0 30 14" width="30" height="14" aria-hidden="true"><rect class="builder__chain-plate" x="2.6" y="3.4" width="14" height="8" rx="4"/><rect class="builder__chain-plate" x="13.6" y="3.4" width="14" height="8" rx="4"/><rect class="builder__chain-line" x="1.5" y="2.5" width="14" height="8" rx="4"/><rect class="builder__chain-line" x="12.5" y="2.5" width="14" height="8" rx="4"/></svg>
            <span class="builder__chain-name">Chain lift</span>
            <span class="builder__chain-state" data-chain-state aria-hidden="true">Off</span>
            ${hint("chain")}
          </button>
          <p class="builder__chain-hint" id="chain-lift-hint" data-chain-hint></p>
        </div>
        <section class="tool-panel__section builder__place" aria-label="Place track">
          <div class="builder__place-actions">
            <button type="button" class="tool-panel__action tool-panel__action--primary" data-append>Place piece ${hint("place")}</button>
            <button type="button" class="tool-panel__action" data-undo>Undo last piece ${hint("undo")}</button>
          </div>
          <p class="builder__test-hint" data-panel-track-height></p>
          <p class="builder__test-hint" data-panel-station-height></p>
        </section>
        <section class="tool-panel__section builder__choices" aria-label="Piece choices" data-choices>
          <h3>Track elements</h3>
          <p class="tool-panel__hint">Common shapes, ready to place.</p>
          <div class="builder__elements" data-elements></div>
          <h3>All pieces</h3>
          <p class="tool-panel__hint">Every single piece, for fine control.</p>
          <div class="builder__palette" role="radiogroup" aria-label="Next piece" data-palette></div>
        </section>
        <section class="tool-panel__section builder__finish" aria-label="Finish and test">
          <h3>Finish and test</h3>
          <div class="builder__actions">
            <button type="button" class="tool-panel__action" data-close>Close circuit ${hint("close")}</button>
            <div class="builder__finish-flow">
              <button type="button" class="tool-panel__action" data-finish aria-describedby="finish-reason">Finish coaster</button>
              <p class="builder__finish-reason" id="finish-reason" data-finish-reason aria-live="polite"></p>
              <div class="builder__finish-confirmation" data-finish-confirmation hidden>
                <p>Review the highlighted connection, then confirm to add it and close the circuit.</p>
                <button type="button" class="tool-panel__action tool-panel__action--primary" data-finish-confirm>Confirm coaster finish</button>
                <button type="button" class="tool-panel__action" data-finish-cancel>Cancel finish preview</button>
              </div>
            </div>
            <button type="button" class="tool-panel__action" data-test>Test coaster ${hint("test")}</button>
            <button type="button" class="tool-panel__action" data-open>Open coaster</button>
          </div>
          <p class="builder__stamp" data-stamp hidden>Circuit complete</p>
          <p class="builder__test-hint" data-test-hint></p>
          <p class="builder__test-hint" data-station-access></p>
          <section aria-label="Coaster testing" data-testing></section>
        </section>
        <section class="tool-panel__section" aria-label="Launch positions" data-launches hidden>
          <h3>Launch positions</h3>
          <ol data-launch-list></ol>
        </section>
        <section class="tool-panel__section" aria-label="Placed track">
          <div class="builder__track-head"><h3>Track pieces</h3><span class="builder__track-count" data-track-count></span></div>
          <ol class="builder__track" data-track></ol>
          <label class="builder__toggle"><input type="checkbox" data-delete-mode> Delete track by clicking it ${hint("deleteMode")}</label>
        </section>
        <details class="tool-panel__section builder__keys"><summary>All keys</summary><dl>
          <dt>${hint("turnLeft")} ${hint("turnRight")}</dt><dd>Turn left or right</dd>
          <dt>${hint("slopeDown")} ${hint("slopeUp")}</dt><dd>Slope down or up</dd>
          <dt>${hint("bankLeft")} ${hint("bankRight")}</dt><dd>Bank left or right</dd>
          <dt>${hint("previousPiece")} ${hint("nextPiece")}</dt><dd>Previous or next piece</dd>
          <dt><kbd>1</kbd>–<kbd>6</kbd></dt><dd>Jump to a group of pieces</dd>
          <dt>${hint("place")} <kbd>B</kbd></dt><dd>Place the piece</dd>
          <dt>${hint("chain")}</dt><dd>Turn the chain lift on or off</dd>
          <dt>${hint("undo")}</dt><dd>Undo the last piece</dd>
          <dt>${hint("deleteMode")}</dt><dd>Delete track by clicking it</dd>
          <dt>${hint("close")} ${hint("test")}</dt><dd>Close the circuit, then test it</dd>
          <dt>${hint("cancel")}</dt><dd>Stop placing the station or deleting</dd>
          <dt><kbd>Q</kbd> <kbd>E</kbd></dt><dd>Rotate the view; arrows pan</dd>
        </dl></details>
      </div>
      <div class="builder__tabpanel" role="tabpanel" id="builder-tabpanel-colours" aria-labelledby="builder-tab-colours" data-builder-tabpanel="colours" hidden>
        <section class="tool-panel__section builder__colors" aria-label="Coaster colours">${COASTER_PARTS.map((part) => `
          <div class="builder__color-row" role="radiogroup" aria-label="${coasterPartNames[part]} colour" data-coaster-color-part="${part}">
            <span class="tool-panel__label">${coasterPartNames[part]}</span>
            <div class="builder__swatches">${COASTER_COLOR_NAMES.map((name) => `<button type="button" role="radio" aria-checked="false" class="builder__swatch" data-coaster-color="${name}" aria-label="${COASTER_COLORS[name].label}" title="${COASTER_COLORS[name].label}" style="--swatch: ${COASTER_COLORS[name].hex}"></button>`).join("")}</div>
          </div>`).join("")}
        </section>
      </div>
      <div class="builder__tabpanel" role="tabpanel" id="builder-tabpanel-operate" aria-labelledby="builder-tab-operate" data-builder-tabpanel="operate" hidden>
        <section class="tool-panel__section builder__coaster-price" aria-label="Ride price">
          <span class="tool-panel__label" id="coaster-price-label">Ride price</span>
          <div class="stepper stepper--compact" role="group" aria-labelledby="coaster-price-label">
            <button type="button" data-coaster-price-step="-1" aria-label="Lower the coaster ticket price">−</button>
            <input type="number" data-coaster-price min="0" max="65535" step="1" value="6" aria-label="Coaster ticket price">
            <button type="button" data-coaster-price-step="1" aria-label="Raise the coaster ticket price">+</button>
          </div>
          <p class="builder__test-hint">Coaster income: <span data-coaster-income>0</span></p>
        </section>
        <section class="tool-panel__section builder__blocks" aria-label="Block sections" data-block-panel>
          <div class="builder__block-heading">
            <h3>Block sections</h3>
            <div class="builder__block-heading-actions">
              <button type="button" class="tool-panel__action" data-auto-blocks>Add block sections</button>
              <button type="button" class="tool-panel__action" data-apply-best-blocks hidden>Use current layout</button>
            </div>
          </div>
          <div class="builder__train-count">
            <span class="tool-panel__label" id="train-count-label">Trains to run</span>
            <div class="stepper stepper--compact" role="group" aria-labelledby="train-count-label">
              <button type="button" data-train-count-step="-1" aria-label="Run one fewer train">−</button>
              <span class="stepper__value" data-train-count aria-live="polite"></span>
              <button type="button" data-train-count-step="1" aria-label="Run one more train">+</button>
            </div>
          </div>
          <p class="builder__block-status" aria-live="polite" data-block-report></p>
          <p class="builder__block-capacity" data-block-capacity></p>
          <p class="builder__block-throughput" data-block-throughput></p>
          <p class="builder__block-message" aria-live="polite" data-block-message hidden></p>
          <ol class="builder__block-list" data-block-sections></ol>
          <datalist id="block-boundary-options" data-block-boundary-options></datalist>
          <p class="tool-panel__hint">Choose an available listed brake boundary. The game keeps the current layout until an edit validates.</p>
          <div class="builder__block-add" data-block-add-controls hidden>
            <label><span class="tool-panel__label" data-block-add-label>New boundary (m)</span><input type="number" min="0" step="any" data-block-add-boundary list="block-boundary-options"></label>
            <button type="button" class="tool-panel__action" data-block-add>Add block</button>
          </div>
          <ul class="builder__block-notes" data-block-notes></ul>
          <ul class="builder__block-suggestions" data-block-suggestions></ul>
        </section>
        <p class="tool-panel__section tool-panel__hint" data-block-panel-hint>Trains and block sections can be set once the circuit is complete and long enough to test.</p>
      </div>
    </div>`;
  const element = <T extends HTMLElement>(selector: string): T => {
    const found = container.querySelector<T>(selector);
    if (!found) throw new Error(`Construction panel is missing ${selector}`);
    return found;
  };
  const height = element<HTMLInputElement>("[data-height]");
  const editor = element("[data-editor]");
  const palette = element("[data-palette]");
  const elements = element("[data-elements]");
  const append = element<HTMLButtonElement>("[data-append]");
  const undo = element<HTMLButtonElement>("[data-undo]");
  const close = element<HTMLButtonElement>("[data-close]");
  const finish = element<HTMLButtonElement>("[data-finish]");
  const finishReason = element("[data-finish-reason]");
  const finishConfirmation = element("[data-finish-confirmation]");
  const confirmFinish = element<HTMLButtonElement>("[data-finish-confirm]");
  const cancelFinish = element<HTMLButtonElement>("[data-finish-cancel]");
  const test = element<HTMLButtonElement>("[data-test]");
  const open = element<HTMLButtonElement>("[data-open]");
  const coasterPrice = element<HTMLInputElement>("[data-coaster-price]");
  const coasterIncome = element("[data-coaster-income]");
  const deleteMode = element<HTMLInputElement>("[data-delete-mode]");
  const chainSection = element("[data-chain-section]");
  const chainToggle = element<HTMLButtonElement>("[data-chain-toggle]");
  const chainState = element("[data-chain-state]");
  const chainHint = element("[data-chain-hint]");
  const message = element("[data-message]");
  const track = element<HTMLOListElement>("[data-track]");
  const stamp = element("[data-stamp]");
  const modelSelect = element<HTMLSelectElement>("[data-model]");
  const modelDescription = element("[data-model-description]");
  const launches = element("[data-launches]");
  const launchList = element<HTMLOListElement>("[data-launch-list]");
  const blockPanel = element("[data-block-panel]");
  const autoBlocks = element<HTMLButtonElement>("[data-auto-blocks]");
  const applyBestBlocks = element<HTMLButtonElement>("[data-apply-best-blocks]");
  const trainCount = element("[data-train-count]");
  const fewerTrains = element<HTMLButtonElement>('[data-train-count-step="-1"]');
  const moreTrains = element<HTMLButtonElement>('[data-train-count-step="1"]');
  const blockReport = element("[data-block-report]");
  const blockCapacity = element("[data-block-capacity]");
  const blockThroughput = element("[data-block-throughput]");
  const blockMessage = element("[data-block-message]");
  const blockSections = element<HTMLOListElement>("[data-block-sections]");
  const blockBoundaryOptions = element<HTMLDataListElement>("[data-block-boundary-options]");
  const addBlockControls = element("[data-block-add-controls]");
  const addBlockBoundary = element<HTMLInputElement>("[data-block-add-boundary]");
  const addBlock = element<HTMLButtonElement>("[data-block-add]");
  const blockNotes = element<HTMLUListElement>("[data-block-notes]");
  const blockSuggestions = element<HTMLUListElement>("[data-block-suggestions]");
  const blockPanelHint = element("[data-block-panel-hint]");
  const choices = element("[data-choices]");
  const tabList = element("[data-builder-tabs]");
  const tabs = builderTabs.map(({ id }) => ({ id, tab: element<HTMLButtonElement>(`[data-builder-tab="${id}"]`), panel: element(`[data-builder-tabpanel="${id}"]`) }));
  let shapes = new Map<string, PieceShape>();
  let state: ConstructionState | undefined;
  let paletteKey = "";
  let elementsKey = "";
  let trackKey = "";
  let blockKey = "";
  let applyBestRequested = false;
  let selectedBlock: number | null = null;
  let closed = false;
  let units: Units = "imperial";

  const shapeOf = (id: string, slope?: string): PieceShape => {
    const shape = (slope && shapes.get(`${id}\0${slope}`)) ?? shapes.get(id);
    if (!shape) throw new Error(`Missing shape for piece ${id}`);
    return shape;
  };
  const select = (piece: string | undefined): void => { if (piece && piece !== state?.selected) dispatch({ type: "construction-select", piece }); };
  const showTab = (shown: BuilderTab, focus: boolean): void => {
    for (const { id, tab, panel } of tabs) {
      tab.setAttribute("aria-selected", String(id === shown));
      tab.tabIndex = id === shown ? 0 : -1;
      panel.hidden = id !== shown;
      if (id === shown && focus) tab.focus();
    }
  };
  showTab("build", false);
  tabList.addEventListener("click", (event) => {
    const tab = (event.target as Element).closest<HTMLElement>("[data-builder-tab]");
    if (tab) showTab(tab.dataset.builderTab as BuilderTab, false);
  });
  tabList.addEventListener("keydown", (event) => {
    const current = tabs.findIndex(({ tab }) => tab.getAttribute("aria-selected") === "true");
    const target = ({ ArrowLeft: current - 1, ArrowRight: current + 1, Home: 0, End: tabs.length - 1 } as Record<string, number | undefined>)[event.key];
    if (target === undefined) return;
    event.preventDefault();
    showTab(tabs[(target + tabs.length) % tabs.length].id, true);
  });
  autoBlocks.addEventListener("click", () => dispatch({ type: "construction-auto-blocks" }));
  applyBestBlocks.addEventListener("click", () => {
    if (state?.blockReport.placementStatus !== "searching") return;
    applyBestRequested = true;
    applyBestBlocks.disabled = true;
    applyBestBlocks.textContent = "Using current layout…";
    dispatch({ type: "construction-apply-best-blocks" });
  });
  for (const [button, step] of [[fewerTrains, -1], [moreTrains, 1]] as const) {
    button.addEventListener("click", () => {
      if (!state) return;
      const count = steppedTrainCount(state.trainCount, state.blockReport.trainCapacity, step);
      if (count !== undefined) dispatch({ type: "construction-set-train-count", count });
    });
  }
  addBlock.addEventListener("click", () => {
    const boundary = addBlockBoundary.valueAsNumber;
    if (Number.isFinite(boundary)) dispatch({ type: "construction-add-block", boundaryMetres: displayToMetres(boundary, units) });
  });
  container.dataset.model = modelSelect.value;
  modelSelect.addEventListener("change", () => {
    const model = coasterModels[modelSelect.value as keyof typeof coasterModels];
    if (!model) throw new Error(`Unknown coaster model ${modelSelect.value}`);
    container.dataset.model = modelSelect.value;
    modelDescription.textContent = model.description;
    dispatch({ type: "construction-set-model", model: model.id });
  });
  let dispatchedHeight = stationHeight.initial;
  const setHeight = (value: number): void => {
    const snapped = Math.min(stationHeight.maximum, Math.max(stationHeight.minimum, Math.round(value / stationHeight.step) * stationHeight.step));
    height.value = String(metresToInput(snapped, units));
    if (snapped !== dispatchedHeight) { dispatchedHeight = snapped; dispatch({ type: "station-height", height: snapped }); }
  };
  height.addEventListener("change", () => setHeight(displayToMetres(Number(height.value), units) || dispatchedHeight));
  container.querySelectorAll<HTMLButtonElement>("[data-height-step]").forEach((button) => button.addEventListener("click", () => setHeight(dispatchedHeight + Number(button.dataset.heightStep) * stationHeight.step)));
  element("[data-start]").addEventListener("click", () => dispatch({ type: "station-placement", enabled: true }));
  append.addEventListener("click", () => dispatch({ type: "construction-append" }));
  undo.addEventListener("click", () => dispatch({ type: "construction-undo" }));
  close.addEventListener("click", () => dispatch({ type: "construction-close" }));
  finish.addEventListener("click", () => dispatch({ type: "construction-finish-preview" }));
  confirmFinish.addEventListener("click", () => dispatch({ type: "construction-finish-confirm" }));
  cancelFinish.addEventListener("click", () => dispatch({ type: "construction-finish-cancel" }));
  test.addEventListener("click", () => dispatch({ type: "construction-test" }));
  open.addEventListener("click", () => dispatch({ type: "construction-set-open", open: !state?.isOpen }));
  const setCoasterPrice = (price: number): void => {
    if (!Number.isInteger(price) || price < 0 || price > 65535) throw new RangeError("Invalid coaster ticket price");
    coasterPrice.value = String(price);
    dispatch({ type: "construction-set-coaster-price", price });
  };
  coasterPrice.addEventListener("change", () => {
    if (coasterPrice.validity.valid && coasterPrice.value !== "") setCoasterPrice(Number(coasterPrice.value));
  });
  const coasterPriceButtons = [...container.querySelectorAll<HTMLButtonElement>("[data-coaster-price-step]")];
  coasterPriceButtons.forEach((button) => button.addEventListener("click", () => {
    setCoasterPrice(Number(coasterPrice.value) + Number(button.dataset.coasterPriceStep));
  }));
  chainToggle.addEventListener("click", () => { if (state) dispatch({ type: "construction-set-chain", enabled: !state.chain }); });
  const colorRows = COASTER_PARTS.map((part) => ({ part, row: element(`[data-coaster-color-part="${part}"]`) }));
  for (const { part, row } of colorRows) {
    row.addEventListener("click", (event) => {
      const swatch = (event.target as Element).closest<HTMLElement>("[data-coaster-color]");
      if (swatch && state?.colors) dispatch({ type: "construction-set-colors", colors: repaint(state.colors, part, swatch.dataset.coasterColor!) });
    });
  }
  deleteMode.addEventListener("change", () => dispatch({ type: "construction-delete-mode", enabled: deleteMode.checked }));
  for (const { attribute } of steppers) for (const step of [-1, 1] as const) {
    element(`[data-stepper="${attribute}-${step}"]`).addEventListener("click", () => { if (state) select(steppedPiece(state.pieces, shapes, state.selected, attribute, step)); });
  }
  palette.addEventListener("click", (event) => {
    const tile = (event.target as Element).closest<HTMLElement>("[data-piece]");
    if (tile) select(tile.dataset.piece);
  });
  elements.addEventListener("click", (event) => {
    const tile = (event.target as Element).closest<HTMLElement>("[data-element]");
    if (tile) select(tile.dataset.element);
  });
  palette.addEventListener("keydown", (event) => {
    const tile = (event.target as Element).closest<HTMLElement>("[data-piece]");
    if (event.key === "Enter" && tile?.dataset.piece === state?.selected) {
      event.preventDefault();
      dispatch({ type: "construction-append" });
      return;
    }
    const step = ({ ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 } as Record<string, -1 | 1 | undefined>)[event.key];
    if (!step || !state) return;
    event.preventDefault();
    select(steppedPiece(state.pieces, shapes, state.selected, "piece", step));
  });

  function renderPalette(pieces: readonly ConstructionPiece[]): void {
    palette.replaceChildren(...groupedPieces(pieces).map(({ group, pieces: members }) => {
      const row = document.createElement("div");
      row.className = "builder__group";
      row.style.setProperty("--plate", groupPlates[group]);
      row.innerHTML = `<h3>${groupNames[group]} <kbd>${groupKeyHint(group)}</kbd></h3><div class="builder__tiles"></div>`;
      row.lastElementChild!.append(...members.map((piece) => {
        const shape = shapeOf(piece.id, piece.slope);
        const displayName = pieceDisplayName(piece);
        const tile = document.createElement("button");
        tile.type = "button";
        tile.className = `piece-tile${piece.refusal ? " piece-tile--refused" : ""}`;
        const levelOut = shape.exitSlope === "Level" && shape.entrySlope !== "Level";
        if (levelOut) tile.classList.add("piece-tile--level-out");
        tile.dataset.piece = piece.id;
        tile.setAttribute("role", "radio");
        tile.setAttribute("aria-label", displayName);
        const tipId = `piece-tip-${piece.id}`;
        tile.setAttribute("aria-describedby", tipId);
        tile.innerHTML = `${pieceIconSvg(shape, iconViews(shape)[0], 46, 34)}${levelOut ? '<span class="piece-tile__level-out">Level out</span>' : ""}<span class="piece-tile__tip" role="tooltip" id="${tipId}"><strong></strong><span></span></span>`;
        tile.querySelector("strong")!.textContent = displayName;
        tile.querySelector(".piece-tile__tip span")!.textContent = piece.refusal;
        return tile;
      }));
      return row;
    }));
  }

  function renderElements(choices: readonly ConstructionElement[]): void {
    elements.replaceChildren(...choices.map((choice) => {
      const shape = shapeOf(choice.pieceId, choice.slope);
      const button = document.createElement("button");
      button.type = "button";
      button.className = "builder__element";
      button.dataset.element = choice.id;
      button.setAttribute("aria-pressed", String(choice.id === state?.selected));
      button.innerHTML = `${pieceIconSvg(shape, iconViews(shape)[0], 46, 34)}<span></span>`;
      button.querySelector("span")!.textContent = choice.name;
      return button;
    }));
  }

  function renderSelection(current: ConstructionState, focusInPalette: boolean): void {
    for (const tile of elements.querySelectorAll<HTMLElement>("[data-element]")) tile.setAttribute("aria-pressed", String(tile.dataset.element === current.selected));
    for (const tile of palette.querySelectorAll<HTMLElement>("[data-piece]")) {
      const checked = tile.dataset.piece === current.selected;
      tile.setAttribute("aria-checked", String(checked));
      tile.tabIndex = checked ? 0 : -1;
      if (checked && focusInPalette) tile.focus();
    }
    const selected = current.pieces.find(({ id }) => id === current.selected);
    const selectedElement = current.elements.find(({ id }) => id === current.selected);
    const preview = element("[data-preview]");
    preview.hidden = !selected && !selectedElement;
    if (selected) {
      const shape = shapeOf(selected.id, selected.slope);
      preview.style.setProperty("--plate", groupPlates[selected.group]);
      element("[data-preview-icons]").innerHTML = iconViews(shape).map((view) => pieceIconSvg(shape, view, 70, 52)).join("");
      element("[data-preview-name]").textContent = pieceDisplayName(selected);
      element("[data-preview-group]").textContent = groupNames[selected.group];
    }
    if (selectedElement) {
      const shape = shapeOf(selectedElement.pieceId, selectedElement.slope);
      preview.style.setProperty("--plate", "var(--teal)");
      element("[data-preview-icons]").innerHTML = iconViews(shape).map((view) => pieceIconSvg(shape, view, 70, 52)).join("");
      element("[data-preview-name]").textContent = selectedElement.name;
      element("[data-preview-group]").textContent = "Track element";
    }
    for (const { attribute } of steppers) {
      const options = stepperOptions(current.pieces, shapes, current.selected, attribute);
      const value = element(`[data-stepper-value="${attribute}"]`);
      const text = options ? options.values[options.index] : "None";
      if (value.textContent !== text) {
        value.textContent = text;
        value.classList.remove("stepper__value--changed");
        void value.offsetWidth;
        value.classList.add("stepper__value--changed");
      }
      element<HTMLButtonElement>(`[data-stepper="${attribute}--1"]`).disabled = !options || options.index <= 0;
      element<HTMLButtonElement>(`[data-stepper="${attribute}-1"]`).disabled = !options || options.index >= options.values.length - 1;
    }
  }

  function renderLaunchPositions(current: ConstructionState): void {
    let distance = 0;
    const positions: { piece: number; distance: number; id: number; launchTargetSpeed: number | null; launchMinimumSpeed: number | null; launchMaximumSpeed: number | null }[] = [];
    current.track.forEach(({ label, slope, id, launchTargetSpeed, launchMinimumSpeed, launchMaximumSpeed }, index) => {
      const shape = shapeOf(label, slope);
      const length = sampledLength(shape);
      if (shape.kind === "Launch") positions.push({ piece: index + 1, distance: distance + length / 2, id, launchTargetSpeed, launchMinimumSpeed, launchMaximumSpeed });
      distance += length;
    });
    launches.hidden = positions.length === 0;
    launchList.replaceChildren(...positions.map(({ piece, distance: launchDistance, id, launchTargetSpeed, launchMinimumSpeed, launchMaximumSpeed }) => {
      const item = document.createElement("li");
      const position = document.createElement("span");
      position.textContent = `Piece ${piece} · ${formatLength(launchDistance, units)} from station`;
      item.append(position);
      if (launchTargetSpeed !== null && launchMinimumSpeed !== null && launchMaximumSpeed !== null) {
        const group = document.createElement("div");
        group.className = "stepper stepper--compact";
        const label = document.createElement("label");
        label.textContent = `Launch ${piece} target (${speedUnitLabel(units)})`;
        const input = document.createElement("input");
        input.type = "number";
        input.min = String(Math.ceil(metresPerSecondToDisplay(launchMinimumSpeed, units)));
        input.max = String(Math.floor(metresPerSecondToDisplay(launchMaximumSpeed, units)));
        input.step = "1";
        input.value = String(Math.round(metresPerSecondToDisplay(launchTargetSpeed, units)));
        input.setAttribute("aria-label", label.textContent);
        input.addEventListener("change", () => {
          if (!Number.isFinite(input.valueAsNumber)) return;
          const speed = Math.max(Number(input.min), Math.min(Number(input.max), input.valueAsNumber));
          dispatch({ type: "construction-set-launch-intensity", id, targetSpeed: displaySpeedToMetresPerSecond(speed, units) });
        });
        label.append(input);
        group.append(label);
        item.append(group);
      }
      return item;
    }));
  }

  function renderTrack(current: ConstructionState, grew: boolean): void {
    track.replaceChildren(...current.track.map(({ id, label, name, group, slope, kind }, index) => {
      const shape = shapeOf(label, slope);
      const displayName = pieceDisplayName({ label: name, group, slope });
      const item = document.createElement("li");
      item.className = `track-chip${grew && index === current.track.length - 1 ? " track-chip--new" : ""}`;
      item.title = displayName;
      item.innerHTML = `<span class="track-chip__number">${index + 1}</span>${pieceIconSvg(shape, iconViews(shape)[0], 26, 20)}<span class="track-chip__name"></span>`;
      item.querySelector(".track-chip__name")!.textContent = displayName;
      if (kind === "Lift") {
        const marker = document.createElement("span");
        marker.dataset.chainMarker = "";
        marker.className = "track-chip__chain";
        marker.textContent = "Chain";
        item.append(marker);
      }
      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "×";
      remove.setAttribute("aria-label", `Delete ${displayName}, piece ${index + 1}`);
      remove.addEventListener("click", () => dispatch({ type: "construction-delete", id }));
      item.append(remove);
      return item;
    }));
    element("[data-track-count]").textContent = current.track.length === 1 ? "1 piece" : `${current.track.length} pieces`;
    renderLaunchPositions(current);
  }

  function renderBlockSections(report: BlockReport, actionsDisabled: boolean): void {
    blockSections.replaceChildren(...report.sections.map((section, index) => {
      const item = document.createElement("li");
      item.className = `builder__block-section${section.editableIndex === selectedBlock ? " builder__block-section--selected" : ""}`;
      item.dataset.blockSection = "";
      item.dataset.blockIndex = section.editableIndex === null ? "" : String(section.editableIndex);
      const heading = document.createElement("strong");
      const trackKindName = blockTrackKindNames[section.trackKind];
      if (!trackKindName) throw new Error(`Unknown block boundary track kind ${section.trackKind}`);
      heading.textContent = section.editable
        ? `Block ${section.editableIndex! + 1} · ${trackKindName}`
        : `Fixed boundary · ${trackKindName}`;
      const boundary = document.createElement("span");
      boundary.className = "builder__block-position";
      boundary.textContent = `${formatLength(section.boundaryMetres, units)} from the station`;
      const reason = document.createElement("p");
      reason.textContent = section.reason;
      item.append(heading, boundary, reason);
      if (section.editable) {
        if (section.editableIndex === null) throw new Error("Editable block section is missing its editable index");
        const label = document.createElement("label");
        label.className = "builder__block-boundary";
        const labelText = document.createElement("span");
        labelText.className = "tool-panel__label";
        labelText.textContent = `Move boundary (${unitLabel(units)})`;
        const input = document.createElement("input");
        input.type = "number";
        input.min = "0";
        input.step = units === "imperial" ? "1" : "any";
        input.value = String(metresToInput(section.boundaryMetres, units));
        input.setAttribute("aria-label", `Block ${section.editableIndex + 1} boundary in ${units === "metric" ? "metres" : "feet"}`);
        input.dataset.blockBoundary = "";
        input.dataset.editable = "true";
        input.dataset.blockIndex = String(section.editableIndex);
        input.setAttribute("list", "block-boundary-options");
        input.disabled = actionsDisabled;
        label.append(labelText, input);
        const actions = document.createElement("div");
        actions.className = "builder__block-actions";
        const move = document.createElement("button");
        move.type = "button";
        move.className = "tool-panel__action";
        move.textContent = "Move block";
        move.dataset.blockMove = String(section.editableIndex);
        move.disabled = actionsDisabled;
        move.addEventListener("click", () => {
          const boundary = input.valueAsNumber;
          if (Number.isFinite(boundary)) dispatch({ type: "construction-move-block", index: section.editableIndex!, boundaryMetres: displayToMetres(boundary, units) });
        });
        const remove = document.createElement("button");
        remove.type = "button";
        remove.className = "tool-panel__action";
        remove.textContent = "Remove block";
        remove.dataset.blockRemove = String(section.editableIndex);
        remove.disabled = actionsDisabled;
        remove.addEventListener("click", () => dispatch({ type: "construction-remove-block", index: section.editableIndex! }));
        actions.append(move, remove);
        item.append(label, actions);
      } else {
        const fixed = document.createElement("span");
        fixed.className = "builder__block-fixed";
        fixed.textContent = "Cannot move or remove";
        item.append(fixed);
      }
      return item;
    }));
  }

  function renderBlockPanel(current: ConstructionState): void {
    const report = current.blockReport;
    if (selectedBlock !== null && !report.sections.some((section) => section.editableIndex === selectedBlock)) selectedBlock = null;
    const view = blockPanelState(report);
    if (!view.canApplyBest) applyBestRequested = false;
    blockReport.textContent = view.statusText;
    blockReport.dataset.placementStatus = report.placementStatus;
    blockCapacity.textContent = view.capacityText;
    blockThroughput.textContent = view.throughputText;
    blockMessage.textContent = view.message;
    blockMessage.hidden = view.message.length === 0;
    autoBlocks.disabled = !current.canTest || view.busy;
    applyBestBlocks.hidden = !view.canApplyBest;
    applyBestBlocks.disabled = applyBestRequested;
    applyBestBlocks.textContent = applyBestRequested ? "Using current layout…" : "Use current layout";
    trainCount.textContent = String(current.trainCount);
    fewerTrains.disabled = report.trainCapacity < current.trainCount || view.busy || current.trainCount <= 1;
    moreTrains.disabled = report.trainCapacity < current.trainCount || view.busy || current.trainCount >= report.trainCapacity;
    const canEdit = current.canTest && report.placementStatus !== "idle" && !view.busy;
    addBlockControls.hidden = !canEdit;
    addBlockBoundary.disabled = !canEdit;
    addBlock.disabled = !canEdit;
    blockNotes.replaceChildren(...report.notes.map((note) => {
      const item = document.createElement("li");
      item.textContent = note;
      return item;
    }));
    blockSuggestions.replaceChildren(...report.suggestions.map((suggestion) => {
      const item = document.createElement("li");
      item.textContent = suggestion;
      return item;
    }));
    blockBoundaryOptions.replaceChildren(...availableBlockBoundaries(current.blockCandidates, report).map((boundaryMetres) => {
      const option = document.createElement("option");
      option.value = String(metresToInput(boundaryMetres, units));
      return option;
    }));
    const nextBlockKey = JSON.stringify([report.sections, report.notes, report.suggestions, report.placementStatus, view.busy, canEdit, view.message, current.message, selectedBlock, units]);
    if (nextBlockKey !== blockKey) {
      blockKey = nextBlockKey;
      renderBlockSections(report, !canEdit);
    }
    blockPanel.dataset.selectedBlock = selectedBlock === null ? "" : String(selectedBlock);
  }

  function renderStationAccess(current: ConstructionState): void {
    const access = current.stationPositions;
    if (access.length !== 0 && access.length !== 6) throw new Error("Invalid station access positions");
    element("[data-station-access]").textContent = access.length ? `Connect a queue at (${access[0]}, ${access[1]}) and a normal exit path at (${access[3]}, ${access[4]}), both at terrain height ${formatLength(access[2], units)}. Connect the queue to the park's normal paths.` : "";
  }

  return {
    setUnits(value) {
      const previousBoundary = addBlockBoundary.valueAsNumber;
      if (Number.isFinite(previousBoundary)) addBlockBoundary.value = String(metresToInput(displayToMetres(previousBoundary, units), value));
      units = value;
      height.min = String(metresToInput(stationHeight.minimum, units));
      height.max = String(metresToInput(stationHeight.maximum, units));
      height.step = String(metresToInput(stationHeight.step, units));
      addBlockBoundary.step = units === "imperial" ? "1" : "any";
      height.setAttribute("aria-label", `Station height above ground (${units === "metric" ? "metres" : "feet"})`);
      element(".builder__station-height .stepper__unit").textContent = unitLabel(units);
      element("[data-block-add-label]").textContent = `New boundary (${unitLabel(units)})`;
      setHeight(dispatchedHeight);
      if (state) { renderLaunchPositions(state); renderBlockPanel(state); renderStationAccess(state); }
    },
    setHeightMarkers(trackHeight, stationHeightText) {
      element("[data-panel-track-height]").textContent = trackHeight;
      element("[data-panel-station-height]").textContent = stationHeightText;
    },
    setShapes(value) {
      const nextShapes = new Map<string, PieceShape>();
      for (const shape of value) {
        if (!nextShapes.has(shape.id)) nextShapes.set(shape.id, shape);
        const variantKey = `${shape.id}\0${shape.exitSlope}`;
        if (nextShapes.has(variantKey)) throw new Error(`Duplicate piece shape variant ${shape.id} at ${shape.exitSlope}`);
        nextShapes.set(variantKey, shape);
      }
      shapes = nextShapes;
    },
    choose(action) {
      if (!state?.active || !state.complete) return;
      select(action.type === "piece-group" ? pieceInGroup(state.pieces, action.group) : steppedPiece(state.pieces, shapes, state.selected, action.attribute, action.step));
    },
    setDeleteMode(enabled) { deleteMode.checked = enabled; },
    setSelectedBlock(index) {
      if (index !== null && !state?.blockReport.sections.some((section) => section.editableIndex === index)) {
        throw new Error(`Unknown editable block section ${index}`);
      }
      selectedBlock = index;
      // A block marker picked in the world opens the settings that edit it.
      if (index !== null) showTab("operate", false);
      if (state) renderBlockPanel(state);
    },
    update(next) {
      const previous = state;
      state = next;
      editor.inert = !next.complete;
      editor.setAttribute("aria-busy", String(!next.complete));
      editor.hidden = !next.active;
      if (next.active && !previous?.active) showTab("build", false);
      blockPanel.hidden = !next.active || (!next.canTest && next.blockReport.sections.length === 0);
      blockPanelHint.hidden = !blockPanel.hidden;
      modelSelect.disabled = next.active;
      container.dataset.selected = next.selected ?? "";
      container.dataset.chain = String(next.chain);
      container.dataset.finishPreview = String(next.finishPreview);
      chainToggle.setAttribute("aria-pressed", String(next.chain));
      for (const { part, row } of colorRows) {
        row.dataset.selected = next.colors?.[part] ?? "";
        for (const swatch of row.querySelectorAll<HTMLElement>("[data-coaster-color]")) swatch.setAttribute("aria-checked", String(swatch.dataset.coasterColor === next.colors?.[part]));
      }
      chainState.textContent = next.chain ? "On" : "Off";
      chainHint.textContent = next.chain
        ? "New pieces are chain lifts. Hatched pieces cannot carry a chain."
        : "Turn on to pull trains up the next pieces.";
      const event = constructionEvent(previous, next);
      if (event === "closed") closed = true;
      else if (previous && previous.track.length !== next.track.length) closed = false;
      stamp.hidden = !closed;
      element("[data-ready]").hidden = !next.canClose;
      element("[data-next]").hidden = closed || next.canClose;
      choices.hidden = closed;
      chainSection.hidden = closed;
      close.classList.toggle("tool-panel__action--primary", next.canClose);
      finish.classList.toggle("tool-panel__action--primary", next.canFinish && !next.canClose);
      append.classList.toggle("tool-panel__action--primary", !next.canClose && !next.canFinish);
      finish.hidden = closed || next.finishPreview;
      finish.disabled = !next.canFinish;
      finishReason.textContent = next.finishReason;
      finishReason.hidden = closed || next.canFinish || next.finishPreview;
      finishConfirmation.hidden = closed || !next.finishPreview;
      stamp.classList.toggle("builder__stamp--fresh", event === "closed");
      const focusInPalette = palette.contains(document.activeElement);
      const nextPaletteKey = next.pieces.map(({ id, refusal }) => `${id}:${refusal}`).join("|");
      if (nextPaletteKey !== paletteKey) { paletteKey = nextPaletteKey; renderPalette(next.pieces); }
      const nextElementsKey = JSON.stringify(next.elements);
      if (nextElementsKey !== elementsKey) { elementsKey = nextElementsKey; renderElements(next.elements); }
      renderSelection(next, focusInPalette);
      const nextTrackKey = next.track.map(({ id, label, kind, launchTargetSpeed }) => `${id}:${label}:${kind}:${launchTargetSpeed}`).join("|");
      if (nextTrackKey !== trackKey) { trackKey = nextTrackKey; renderTrack(next, event === "placed"); }
      append.disabled = !next.canAppend;
      undo.disabled = next.track.length === 0;
      close.disabled = !next.canClose;
      test.disabled = !next.canTest;
      open.disabled = !next.canTest && !next.isOpen;
      open.textContent = next.isOpen ? "Close coaster" : "Open coaster";
      if (document.activeElement !== coasterPrice) coasterPrice.value = String(next.coasterPrice);
      for (const button of coasterPriceButtons) {
        const priceChange = Number(button.dataset.coasterPriceStep);
        button.disabled = priceChange < 0 ? next.coasterPrice === 0 : next.coasterPrice === 65535;
      }
      coasterIncome.textContent = String(next.coasterIncome);
      renderStationAccess(next);
      element("[data-test-hint]").textContent = !next.canTest && !next.testingReport ? "Complete the circuit and make it longer than the train to test your coaster." : "";
      message.textContent = next.message;
      renderBlockPanel(next);
    },
  };
}
