import type { ConstructionState } from "./construction-protocol";
import type { InputAction } from "./input/actions";

/** Native controls keep every construction action available to keyboard and assistive technology users. */
export function createConstructionPanel(container: HTMLElement, dispatch: (action: InputAction) => void): { update(state: ConstructionState): void } {
  container.innerHTML = `
    <h2>Build a coaster</h2>
    <button type="button" data-start>Place station</button>
    <label>Station height above ground (metres) <select data-height></select></label>
    <p>Choose a tile, then point and click to choose the station direction.</p>
    <p role="status" data-message></p>
    <div data-editor hidden>
      <p>With the world focused: B places, Backspace undoes, C closes, and T tests.</p>
      <div data-steppers></div>
      <label>Next piece <select data-piece aria-label="Next piece"></select></label>
      <button type="button" data-append>Place piece</button>
      <button type="button" data-undo>Undo last piece</button>
      <button type="button" data-close>Close circuit</button>
      <button type="button" data-test>Test coaster</button>
      <label><input type="checkbox" data-delete-mode> Delete track by clicking it</label>
      <h3>Track pieces</h3><ol data-track></ol>
    </div>`;
  const start = container.querySelector<HTMLButtonElement>("[data-start]")!;
  const height = container.querySelector<HTMLSelectElement>("[data-height]")!;
  for (let value = 4; value <= 100; value += 2) height.add(new Option(String(value), String(value)));
  height.value = "12";
  const editor = container.querySelector<HTMLElement>("[data-editor]")!;
  const piece = container.querySelector<HTMLSelectElement>("[data-piece]")!;
  const append = container.querySelector<HTMLButtonElement>("[data-append]")!;
  const undo = container.querySelector<HTMLButtonElement>("[data-undo]")!;
  const close = container.querySelector<HTMLButtonElement>("[data-close]")!;
  const test = container.querySelector<HTMLButtonElement>("[data-test]")!;
  const deleteMode = container.querySelector<HTMLInputElement>("[data-delete-mode]")!;
  const steppers = container.querySelector<HTMLElement>("[data-steppers]")!;
  const message = container.querySelector<HTMLElement>("[data-message]")!;
  const track = container.querySelector<HTMLOListElement>("[data-track]")!;
  start.addEventListener("click", () => dispatch({ type: "station-placement", enabled: true }));
  height.addEventListener("change", () => dispatch({ type: "station-height", height: Number(height.value) }));
  piece.addEventListener("change", () => dispatch({ type: "construction-select", piece: piece.value }));
  append.addEventListener("click", () => dispatch({ type: "construction-append" }));
  undo.addEventListener("click", () => dispatch({ type: "construction-undo" }));
  close.addEventListener("click", () => dispatch({ type: "construction-close" }));
  deleteMode.addEventListener("change", () => dispatch({ type: "construction-delete-mode", enabled: deleteMode.checked }));
  test.addEventListener("click", () => dispatch({ type: "construction-test" }));
  return {
    update(state) {
      editor.hidden = !state.active;
      container.dataset.selected = state.selected ?? "";
      const selectedPiece = state.pieces.find((candidate) => candidate.id === state.selected);
      const groups = new Map<string, HTMLOptGroupElement>();
      for (const choice of state.pieces) {
        let group = groups.get(choice.group);
        if (!group) {
          group = document.createElement("optgroup");
          group.label = choice.group;
          groups.set(choice.group, group);
        }
        group.append(new Option(choice.label, choice.id));
      }
      piece.replaceChildren(...groups.values());
      const focusedStepper = steppers.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.stepper : undefined;
      steppers.replaceChildren(...(["heading", "slope", "bank"] as const).map((attribute) => {
        const wrapper = document.createElement("div");
        const label = attribute === "heading" ? "Direction" : attribute === "slope" ? "Slope" : "Bank";
        const values = [...new Set(state.pieces.filter((candidate) => selectedPiece &&
          (["heading", "slope", "bank"] as const).every((other) => other === attribute || candidate[other] === selectedPiece[other])
        ).map((candidate) => candidate[attribute]))];
        const output = document.createElement("span");
        output.textContent = `${label}: ${selectedPiece?.[attribute] ?? "None"}`;
        const button = (step: number) => {
          const control = document.createElement("button");
          control.type = "button";
          control.dataset.stepper = `${attribute}-${step}`;
          control.textContent = step < 0 ? "−" : "+";
          control.setAttribute("aria-label", `${step < 0 ? "Previous" : "Next"} ${label.toLowerCase()}`);
          control.disabled = values.length < 2;
          control.addEventListener("click", () => {
            if (!selectedPiece) return;
            const next = values[(values.indexOf(selectedPiece[attribute]) + step + values.length) % values.length];
            const choice = state.pieces.find((candidate) => candidate[attribute] === next &&
              (["heading", "slope", "bank"] as const).every((other) => other === attribute || candidate[other] === selectedPiece[other]));
            if (choice) dispatch({ type: "construction-select", piece: choice.id });
          });
          return control;
        };
        wrapper.append(button(-1), output, button(1));
        return wrapper;
      }));
      if (focusedStepper) steppers.querySelector<HTMLButtonElement>(`[data-stepper="${focusedStepper}"]`)?.focus();
      piece.value = state.selected ?? "";
      piece.disabled = state.pieces.length === 0;
      append.disabled = !state.canAppend;
      undo.disabled = state.track.length === 0;
      close.disabled = !state.canClose;
      test.disabled = !state.canTest;
      message.textContent = state.message;
      track.replaceChildren(...state.track.map(({ id, label }, index) => {
        const item = document.createElement("li");
        item.append(document.createTextNode(`${label} `));
        const remove = document.createElement("button");
        remove.type = "button";
        remove.textContent = `Delete ${label}`;
        remove.setAttribute("aria-label", `Delete ${label}, piece ${index + 1}`);
        remove.addEventListener("click", () => dispatch({ type: "construction-delete", id }));
        item.append(remove);
        return item;
      }));
    },
  };
}
