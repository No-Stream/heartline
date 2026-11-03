import { constructionEvent } from "./construction-effects";
import { availableBlockBoundaries, blockPanelState, steppedTrainCount } from "./block-panel-state";
import { groupedPieces, pieceInGroup, steppedPiece, stepperOptions, type StepAttribute } from "./construction-choices";
import type { BlockReport, ConstructionPiece, ConstructionState, PieceShape } from "./construction-protocol";
import type { InputAction } from "./input/actions";
import { builderShortcuts, groupKeyHint, keyHint } from "./input/bindings";
import { iconViews, pieceIconSvg } from "./piece-icons";
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
const stationHeight = { minimum: 4, maximum: 100, step: 2, initial: 12 };
const steppers: readonly { attribute: StepAttribute; label: string; previous: string; next: string; keys: readonly [keyof typeof builderShortcuts, keyof typeof builderShortcuts] }[] = [
  { attribute: "turn", label: "Direction", previous: "Turn left", next: "Turn right", keys: ["turnLeft", "turnRight"] },
  { attribute: "slope", label: "Slope", previous: "Slope down", next: "Slope up", keys: ["slopeDown", "slopeUp"] },
  { attribute: "bank", label: "Bank", previous: "Bank left", next: "Bank right", keys: ["bankLeft", "bankRight"] },
];
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
      <section class="tool-panel__section builder__palette" role="radiogroup" aria-label="Next piece" data-palette></section>
      <section class="tool-panel__section" aria-label="Track actions">
        <div class="builder__actions">
          <div class="builder__ride-controls">
            <div class="builder__coaster-price">
              <span class="tool-panel__label" id="coaster-price-label">Ride price</span>
              <div class="stepper stepper--compact" role="group" aria-labelledby="coaster-price-label">
                <button type="button" data-coaster-price-step="-1" aria-label="Lower the coaster ticket price">−</button>
                <input type="number" data-coaster-price min="0" max="65535" step="1" value="6" aria-label="Coaster ticket price">
                <button type="button" data-coaster-price-step="1" aria-label="Raise the coaster ticket price">+</button>
              </div>
            </div>
            <button type="button" class="tool-panel__action" data-open>Open coaster</button>
          </div>
          <button type="button" class="tool-panel__action tool-panel__action--primary" data-append>Place piece ${hint("place")}</button>
          <button type="button" class="tool-panel__action" data-undo>Undo last piece ${hint("undo")}</button>
          <button type="button" class="tool-panel__action" data-close>Close circuit ${hint("close")}</button>
          <button type="button" class="tool-panel__action" data-test>Test coaster ${hint("test")}</button>
        </div>
        <p class="builder__test-hint">Coaster income: <span data-coaster-income>0</span></p>
        <label class="builder__toggle"><input type="checkbox" data-delete-mode> Delete track by clicking it ${hint("deleteMode")}</label>
      </section>
      <section class="tool-panel__section" aria-label="Circuit">
        <p class="builder__stamp" data-stamp hidden>Circuit complete</p>
        <p class="builder__test-hint" data-test-hint></p>
        <p class="builder__test-hint" data-station-access></p>
        <section aria-label="Coaster testing" data-testing></section>
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
          <label><span class="tool-panel__label">New boundary (m)</span><input type="number" min="0" step="any" data-block-add-boundary list="block-boundary-options"></label>
          <button type="button" class="tool-panel__action" data-block-add>Add block</button>
        </div>
        <ul class="builder__block-notes" data-block-notes></ul>
        <ul class="builder__block-suggestions" data-block-suggestions></ul>
      </section>
      <section class="tool-panel__section" aria-label="Launch positions" data-launches hidden>
        <h3>Launch positions</h3>
        <ol data-launch-list></ol>
      </section>
      <section class="tool-panel__section" aria-label="Placed track">
        <div class="builder__track-head"><h3>Track pieces</h3><span class="builder__track-count" data-track-count></span></div>
        <ol class="builder__track" data-track></ol>
      </section>
      <details class="tool-panel__section builder__keys"><summary>All keys</summary><dl>
        <dt>${hint("turnLeft")} ${hint("turnRight")}</dt><dd>Turn left or right</dd>
        <dt>${hint("slopeDown")} ${hint("slopeUp")}</dt><dd>Slope down or up</dd>
        <dt>${hint("bankLeft")} ${hint("bankRight")}</dt><dd>Bank left or right</dd>
        <dt>${hint("previousPiece")} ${hint("nextPiece")}</dt><dd>Previous or next piece</dd>
        <dt><kbd>1</kbd>–<kbd>6</kbd></dt><dd>Jump to a group of pieces</dd>
        <dt>${hint("place")} <kbd>B</kbd></dt><dd>Place the piece</dd>
        <dt>${hint("undo")}</dt><dd>Undo the last piece</dd>
        <dt>${hint("deleteMode")}</dt><dd>Delete track by clicking it</dd>
        <dt>${hint("close")} ${hint("test")}</dt><dd>Close the circuit, then test it</dd>
        <dt>${hint("cancel")}</dt><dd>Stop placing the station or deleting</dd>
        <dt><kbd>Q</kbd> <kbd>E</kbd></dt><dd>Rotate the view; arrows pan</dd>
      </dl></details>
    </div>`;
  const element = <T extends HTMLElement>(selector: string): T => {
    const found = container.querySelector<T>(selector);
    if (!found) throw new Error(`Construction panel is missing ${selector}`);
    return found;
  };
  const height = element<HTMLInputElement>("[data-height]");
  const editor = element("[data-editor]");
  const palette = element("[data-palette]");
  const append = element<HTMLButtonElement>("[data-append]");
  const undo = element<HTMLButtonElement>("[data-undo]");
  const close = element<HTMLButtonElement>("[data-close]");
  const test = element<HTMLButtonElement>("[data-test]");
  const open = element<HTMLButtonElement>("[data-open]");
  const coasterPrice = element<HTMLInputElement>("[data-coaster-price]");
  const coasterIncome = element("[data-coaster-income]");
  const deleteMode = element<HTMLInputElement>("[data-delete-mode]");
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
  let shapes = new Map<string, PieceShape>();
  let state: ConstructionState | undefined;
  let paletteKey = "";
  let trackKey = "";
  let blockKey = "";
  let applyBestRequested = false;
  let selectedBlock: number | null = null;
  let closed = false;

  const shapeOf = (id: string): PieceShape => {
    const shape = shapes.get(id);
    if (!shape) throw new Error(`Missing shape for piece ${id}`);
    return shape;
  };
  const select = (piece: string | undefined): void => { if (piece && piece !== state?.selected) dispatch({ type: "construction-select", piece }); };
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
    const boundaryMetres = addBlockBoundary.valueAsNumber;
    if (Number.isFinite(boundaryMetres)) dispatch({ type: "construction-add-block", boundaryMetres });
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
    height.value = String(snapped);
    if (snapped !== dispatchedHeight) { dispatchedHeight = snapped; dispatch({ type: "station-height", height: snapped }); }
  };
  height.addEventListener("change", () => setHeight(Number(height.value) || dispatchedHeight));
  container.querySelectorAll<HTMLButtonElement>("[data-height-step]").forEach((button) => button.addEventListener("click", () => setHeight(dispatchedHeight + Number(button.dataset.heightStep) * stationHeight.step)));
  element("[data-start]").addEventListener("click", () => dispatch({ type: "station-placement", enabled: true }));
  append.addEventListener("click", () => dispatch({ type: "construction-append" }));
  undo.addEventListener("click", () => dispatch({ type: "construction-undo" }));
  close.addEventListener("click", () => dispatch({ type: "construction-close" }));
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
  deleteMode.addEventListener("change", () => dispatch({ type: "construction-delete-mode", enabled: deleteMode.checked }));
  for (const { attribute } of steppers) for (const step of [-1, 1] as const) {
    element(`[data-stepper="${attribute}-${step}"]`).addEventListener("click", () => { if (state) select(steppedPiece(state.pieces, shapes, state.selected, attribute, step)); });
  }
  palette.addEventListener("click", (event) => {
    const tile = (event.target as Element).closest<HTMLElement>("[data-piece]");
    if (tile) select(tile.dataset.piece);
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
        const shape = shapeOf(piece.id);
        const tile = document.createElement("button");
        tile.type = "button";
        tile.className = `piece-tile${piece.refusal ? " piece-tile--refused" : ""}`;
        const levelOut = shape.exitSlope === "Level" && shape.entrySlope !== "Level";
        if (levelOut) tile.classList.add("piece-tile--level-out");
        tile.dataset.piece = piece.id;
        tile.setAttribute("role", "radio");
        tile.setAttribute("aria-label", shape.name);
        const tipId = `piece-tip-${piece.id}`;
        tile.setAttribute("aria-describedby", tipId);
        tile.innerHTML = `${pieceIconSvg(shape, iconViews(shape)[0], 46, 34)}${levelOut ? '<span class="piece-tile__level-out">Level out</span>' : ""}<span class="piece-tile__tip" role="tooltip" id="${tipId}"><strong></strong><span></span></span>`;
        tile.querySelector("strong")!.textContent = shape.name;
        tile.querySelector(".piece-tile__tip span")!.textContent = piece.refusal;
        return tile;
      }));
      return row;
    }));
  }

  function renderSelection(current: ConstructionState, focusInPalette: boolean): void {
    for (const tile of palette.querySelectorAll<HTMLElement>("[data-piece]")) {
      const checked = tile.dataset.piece === current.selected;
      tile.setAttribute("aria-checked", String(checked));
      tile.tabIndex = checked ? 0 : -1;
      if (checked && focusInPalette) tile.focus();
    }
    const selected = current.pieces.find(({ id }) => id === current.selected);
    const preview = element("[data-preview]");
    preview.hidden = !selected;
    if (selected) {
      const shape = shapeOf(selected.id);
      preview.style.setProperty("--plate", groupPlates[selected.group]);
      element("[data-preview-icons]").innerHTML = iconViews(shape).map((view) => pieceIconSvg(shape, view, 70, 52)).join("");
      element("[data-preview-name]").textContent = shape.name;
      element("[data-preview-group]").textContent = groupNames[selected.group];
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
    const positions: { piece: number; distance: number }[] = [];
    current.track.forEach(({ label }, index) => {
      const shape = shapeOf(label);
      const length = sampledLength(shape);
      if (shape.kind === "Launch") positions.push({ piece: index + 1, distance: distance + length / 2 });
      distance += length;
    });
    launches.hidden = positions.length === 0;
    launchList.replaceChildren(...positions.map(({ piece, distance: launchDistance }) => {
      const item = document.createElement("li");
      item.textContent = `Piece ${piece} · ${launchDistance.toFixed(1)} m from station`;
      return item;
    }));
  }

  function renderTrack(current: ConstructionState, grew: boolean): void {
    track.replaceChildren(...current.track.map(({ id, label }, index) => {
      const shape = shapeOf(label);
      const item = document.createElement("li");
      item.className = `track-chip${grew && index === current.track.length - 1 ? " track-chip--new" : ""}`;
      item.title = shape.name;
      item.innerHTML = `<span class="track-chip__number">${index + 1}</span>${pieceIconSvg(shape, iconViews(shape)[0], 26, 20)}<span class="track-chip__name"></span>`;
      item.querySelector(".track-chip__name")!.textContent = shape.name;
      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "×";
      remove.setAttribute("aria-label", `Delete ${shape.name}, piece ${index + 1}`);
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
      boundary.textContent = `${section.boundaryMetres.toFixed(1)} m from the station`;
      const reason = document.createElement("p");
      reason.textContent = section.reason;
      item.append(heading, boundary, reason);
      if (section.editable) {
        if (section.editableIndex === null) throw new Error("Editable block section is missing its editable index");
        const label = document.createElement("label");
        label.className = "builder__block-boundary";
        const labelText = document.createElement("span");
        labelText.className = "tool-panel__label";
        labelText.textContent = "Move boundary (m)";
        const input = document.createElement("input");
        input.type = "number";
        input.min = "0";
        input.step = "any";
        input.value = String(section.boundaryMetres);
        input.setAttribute("aria-label", `Block ${section.editableIndex + 1} boundary in metres`);
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
          const boundaryMetres = input.valueAsNumber;
          if (Number.isFinite(boundaryMetres)) dispatch({ type: "construction-move-block", index: section.editableIndex!, boundaryMetres });
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
      option.value = String(boundaryMetres);
      return option;
    }));
    const nextBlockKey = JSON.stringify([report.sections, report.notes, report.suggestions, report.placementStatus, view.busy, canEdit, view.message, current.message, selectedBlock]);
    if (nextBlockKey !== blockKey) {
      blockKey = nextBlockKey;
      renderBlockSections(report, !canEdit);
    }
    blockPanel.dataset.selectedBlock = selectedBlock === null ? "" : String(selectedBlock);
  }

  return {
    setShapes(value) {
      const nextShapes = new Map<string, PieceShape>();
      for (const shape of value) {
        if (nextShapes.has(shape.id)) throw new Error(`Duplicate piece shape ID ${shape.id}`);
        nextShapes.set(shape.id, shape);
      }
      shapes = nextShapes;
    },
    choose(action) {
      if (!state?.active) return;
      select(action.type === "piece-group" ? pieceInGroup(state.pieces, action.group) : steppedPiece(state.pieces, shapes, state.selected, action.attribute, action.step));
    },
    setDeleteMode(enabled) { deleteMode.checked = enabled; },
    setSelectedBlock(index) {
      if (index !== null && !state?.blockReport.sections.some((section) => section.editableIndex === index)) {
        throw new Error(`Unknown editable block section ${index}`);
      }
      selectedBlock = index;
      if (state) renderBlockPanel(state);
    },
    update(next) {
      const previous = state;
      state = next;
      editor.hidden = !next.active;
      blockPanel.hidden = !next.active || (!next.canTest && next.blockReport.sections.length === 0);
      modelSelect.disabled = next.active;
      container.dataset.selected = next.selected ?? "";
      const event = constructionEvent(previous, next);
      if (event === "closed") closed = true;
      else if (previous && previous.track.length !== next.track.length) closed = false;
      stamp.hidden = !closed;
      element("[data-ready]").hidden = !next.canClose;
      element("[data-next]").hidden = closed || next.canClose;
      palette.hidden = closed;
      close.classList.toggle("tool-panel__action--primary", next.canClose);
      append.classList.toggle("tool-panel__action--primary", !next.canClose);
      stamp.classList.toggle("builder__stamp--fresh", event === "closed");
      const focusInPalette = palette.contains(document.activeElement);
      const nextPaletteKey = next.pieces.map(({ id, refusal }) => `${id}:${refusal}`).join("|");
      if (nextPaletteKey !== paletteKey) { paletteKey = nextPaletteKey; renderPalette(next.pieces); }
      renderSelection(next, focusInPalette);
      const nextTrackKey = next.track.map(({ id, label }) => `${id}:${label}`).join("|");
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
      const access = next.stationPositions;
      if (access.length !== 0 && access.length !== 6) throw new Error("Invalid station access positions");
      element("[data-station-access]").textContent = access.length ? `Connect a queue at (${access[0]}, ${access[1]}) and a normal exit path at (${access[3]}, ${access[4]}), both at terrain height ${access[2]}. Connect the queue to the park's normal paths.` : "";
      element("[data-test-hint]").textContent = !next.canTest && !next.testingReport ? "Complete the circuit and make it longer than the train to test your coaster." : "";
      message.textContent = next.message;
      renderBlockPanel(next);
    },
  };
}
