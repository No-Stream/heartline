import { COASTER_MODELS } from "./generated/coaster-models";
import type { CoasterListEntry } from "./protocol";
import "./print-modern.css";
import "./coaster-list.css";

export interface CoasterListItem {
  readonly id: number;
  readonly name: string;
  readonly model: string;
  readonly status: string;
}

export interface CoasterListActions {
  onSelect(id: number): void;
  onNewCoaster(): void;
}

export interface CoasterList {
  update(coasters: readonly CoasterListEntry[]): void;
}

export function coasterListItems(coasters: readonly Pick<CoasterListEntry, "id" | "name" | "model" | "open" | "complete">[]): CoasterListItem[] {
  return coasters.map((coaster) => {
    const model = COASTER_MODELS.find(({ key }) => key === coaster.model);
    if (!model) throw new Error(`Unknown coaster model ${coaster.model}`);
    const status = coaster.complete ? (coaster.open ? "Open" : "Closed") : "Closed · Incomplete";
    return { id: coaster.id, name: coaster.name, model: model.name, status };
  });
}

export function createCoasterList(container: HTMLElement, actions: CoasterListActions): CoasterList {
  container.classList.add("tool-panel", "coaster-list");
  container.innerHTML = `<header class="tool-panel__header">
      <div><p class="tool-panel__eyebrow">Park rides</p><h2>Coasters</h2></div>
    </header>
    <section class="tool-panel__section coaster-list__section" aria-label="Park coasters">
      <p class="tool-panel__hint" data-coaster-list-empty>No coasters in this park yet.</p>
      <ul class="coaster-list__items" data-coaster-list></ul>
      <button type="button" class="tool-panel__action tool-panel__action--primary coaster-list__new" data-new-coaster>New coaster</button>
    </section>`;
  const list = container.querySelector<HTMLUListElement>("[data-coaster-list]");
  const empty = container.querySelector<HTMLElement>("[data-coaster-list-empty]");
  const newButton = container.querySelector<HTMLButtonElement>("[data-new-coaster]");
  if (!list || !empty || !newButton) throw new Error("Coaster list is missing required elements");

  newButton.addEventListener("click", actions.onNewCoaster);
  let renderedKey = "";
  return {
    update(coasters) {
      const items = coasterListItems(coasters);
      const nextKey = JSON.stringify(items);
      if (nextKey === renderedKey) return;
      renderedKey = nextKey;
      empty.hidden = items.length > 0;
      list.replaceChildren(...items.map((item) => {
        const row = document.createElement("li");
        row.className = "coaster-list__item";
        const select = document.createElement("button");
        select.type = "button";
        select.className = "coaster-list__ride";
        select.dataset.coasterId = String(item.id);
        select.setAttribute("aria-label", `View ${item.name}, ${item.model}, ${item.status}`);
        select.addEventListener("click", () => actions.onSelect(item.id));
        const details = document.createElement("span");
        details.className = "coaster-list__details";
        const name = document.createElement("span");
        name.className = "coaster-list__name";
        name.textContent = item.name;
        const model = document.createElement("span");
        model.className = "coaster-list__model";
        model.textContent = item.model;
        details.append(name, model);
        const status = document.createElement("span");
        status.className = "coaster-list__status";
        status.classList.toggle("coaster-list__status--closed", item.status !== "Open");
        status.textContent = item.status;
        select.append(details, status);
        row.append(select);
        return row;
      }));
    },
  };
}
