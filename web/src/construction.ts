import { constructionEvent } from "./construction-effects";
import { groupedPieces, pieceInGroup, steppedPiece, stepperOptions, type StepAttribute } from "./construction-choices";
import type { ConstructionPiece, ConstructionState, PieceShape } from "./construction-protocol";
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
}

const groupNames: Record<string, string> = { straight: "Straight", turn: "Turns", slope: "Slopes", bank: "Banks", inversion: "Inversions", special: "Special" };
const groupPlates: Record<string, string> = { straight: "var(--slate)", turn: "var(--teal)", slope: "var(--mustard)", bank: "var(--coral)", inversion: "var(--coral)", special: "var(--slate)" };
const coasterModels = {
  "model:hybrid": {
    name: "Timber Tumbler",
    manufacturer: "Rocky Road Construction",
    description: "Steel box track on a wooden structure.",
  },
  "model:velocity_viper": {
    name: "Velocity Viper",
    manufacturer: "Impulse Engineering",
    description: "Multi-launch steel track and columns with short, standard, and long launch sections.",
  },
  "model:wooden_twister": {
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
          <button type="button" class="tool-panel__action tool-panel__action--primary" data-append>Place piece ${hint("place")}</button>
          <button type="button" class="tool-panel__action" data-undo>Undo last piece ${hint("undo")}</button>
          <button type="button" class="tool-panel__action" data-close>Close circuit ${hint("close")}</button>
          <button type="button" class="tool-panel__action" data-test>Test coaster ${hint("test")}</button>
          <button type="button" class="tool-panel__action" data-open>Open coaster</button>
        </div>
        <label class="builder__toggle"><input type="checkbox" data-delete-mode> Delete track by clicking it ${hint("deleteMode")}</label>
      </section>
      <section class="tool-panel__section" aria-label="Circuit">
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
  const deleteMode = element<HTMLInputElement>("[data-delete-mode]");
  const message = element("[data-message]");
  const track = element<HTMLOListElement>("[data-track]");
  const stamp = element("[data-stamp]");
  const modelSelect = element<HTMLSelectElement>("[data-model]");
  const modelDescription = element("[data-model-description]");
  const launches = element("[data-launches]");
  const launchList = element<HTMLOListElement>("[data-launch-list]");
  let shapes = new Map<string, PieceShape>();
  let state: ConstructionState | undefined;
  let paletteKey = "";
  let trackKey = "";
  let closed = false;

  const shapeOf = (id: string): PieceShape => {
    const shape = shapes.get(id);
    if (!shape) throw new Error(`Missing shape for piece ${id}`);
    return shape;
  };
  const select = (piece: string | undefined): void => { if (piece && piece !== state?.selected) dispatch({ type: "construction-select", piece }); };
  container.dataset.model = modelSelect.value;
  modelSelect.addEventListener("change", () => {
    const model = coasterModels[modelSelect.value as keyof typeof coasterModels];
    if (!model) throw new Error(`Unknown coaster model ${modelSelect.value}`);
    container.dataset.model = modelSelect.value;
    modelDescription.textContent = model.description;
    dispatch({ type: "construction-select", piece: modelSelect.value });
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
        tile.dataset.piece = piece.id;
        tile.setAttribute("role", "radio");
        tile.setAttribute("aria-label", shape.name);
        const tipId = `piece-tip-${piece.id}`;
        tile.setAttribute("aria-describedby", tipId);
        tile.innerHTML = `${pieceIconSvg(shape, iconViews(shape)[0], 46, 34)}<span class="piece-tile__tip" role="tooltip" id="${tipId}"><strong></strong><span></span></span>`;
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
    update(next) {
      const previous = state;
      state = next;
      editor.hidden = !next.active;
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
      const access = next.stationPositions;
      if (access.length !== 0 && access.length !== 6) throw new Error("Invalid station access positions");
      element("[data-station-access]").textContent = access.length ? `Connect a queue at (${access[0]}, ${access[1]}) and a normal exit path at (${access[3]}, ${access[4]}), both at terrain height ${access[2]}. Connect the queue to the park's normal paths.` : "";
      element("[data-test-hint]").textContent = !next.canTest && !next.testingReport ? "Complete the circuit and make it longer than the train to test your coaster." : "";
      message.textContent = next.message;
    },
  };
}
