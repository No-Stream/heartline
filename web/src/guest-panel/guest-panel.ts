import { activityLabel, memoryLabel, relationshipLabel, thoughtLabel, traitSummary } from "./presentation";
import type { InputAction } from "../input/actions";
import type { GuestDetail } from "./protocol";
import { formatLength, type Units } from "../units";
import { canKeepGuestPanelPosition, GUEST_PANEL_EDGE as EDGE, GUEST_PANEL_GAP as GAP } from "./placement";
import "./guest-panel.css";

export interface GuestPanelCallbacks {
  readonly dispatch: (action: InputAction) => void;
}

export interface GuestPanelAnchor {
  /** Coordinates in the canvas backing store. */
  readonly x: number;
  readonly y: number;
}

export interface GuestPanel {
  setUnits(units: Units): void;
  update(detail: GuestDetail): void;
  setAnchor(anchor: GuestPanelAnchor | null): void;
  close(): void;
}

type Side = "right" | "left" | "above" | "below";

interface Placement {
  readonly side: Side;
  readonly availableWidth: number;
  readonly availableHeight: number;
  readonly score: number;
}

const MAX_PANEL_WIDTH = 304;
const AGE_LABELS = { child: "Child", adult: "Adult", senior: "Senior" } as const;
const NEEDS = [
  ["Hunger", "hunger"],
  ["Thirst", "thirst"],
  ["Restroom", "restroom"],
  ["Nausea", "nausea"],
] as const;

/** Creates an accessible DOM panel that stays beside its selected guest. */
export function createGuestPanel(
  canvas: HTMLCanvasElement,
  callbacks: GuestPanelCallbacks,
): GuestPanel {
  const root = document.createElement("aside");
  root.dataset.testid = "guest-panel";
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-label", "Guest details");
  root.hidden = true;
  root.innerHTML = `
    <header class="guest-panel__header">
      <p class="guest-panel__eyebrow" data-panel-eyebrow></p>
      <h2 data-testid="guest-name"></h2>
      <button class="guest-panel__close" type="button" data-testid="guest-panel-close" aria-label="Close guest details">×</button>
    </header>
    <p class="guest-panel__meta" data-panel-meta></p>
    <section class="guest-panel__section" aria-labelledby="guest-panel-activity-heading">
      <h3 id="guest-panel-activity-heading">Right now</h3>
      <p class="guest-panel__activity" data-panel-activity></p>
      <p class="guest-panel__destination" data-panel-destination></p>
    </section>
    <section class="guest-panel__section" data-testid="guest-needs" aria-labelledby="guest-panel-needs-heading">
      <h3 id="guest-panel-needs-heading">Needs</h3>
      <div class="guest-panel__needs" data-panel-needs></div>
      <p class="guest-panel__destination" data-panel-energy></p>
    </section>
    <section class="guest-panel__section" data-testid="guest-thoughts" aria-labelledby="guest-panel-thoughts-heading">
      <h3 id="guest-panel-thoughts-heading">Thinking right now</h3>
      <p class="guest-panel__thought" data-panel-thought></p>
    </section>
    <section class="guest-panel__section" aria-labelledby="guest-panel-group-heading">
      <h3 id="guest-panel-group-heading">Here with</h3>
      <ul class="guest-panel__group" data-panel-group></ul>
    </section>
    <section class="guest-panel__section" aria-labelledby="guest-panel-traits-heading">
      <h3 id="guest-panel-traits-heading">A little about them</h3>
      <ul class="guest-panel__traits" data-panel-traits></ul>
    </section>
    <section class="guest-panel__section" aria-labelledby="guest-panel-memories-heading">
      <h3 id="guest-panel-memories-heading">Recent memories</h3>
      <ul class="guest-panel__memories" data-panel-memories></ul>
    </section>`;
  document.body.append(root);

  const name = root.querySelector<HTMLElement>("[data-testid='guest-name']")!;
  const eyebrow = root.querySelector<HTMLElement>("[data-panel-eyebrow]")!;
  const meta = root.querySelector<HTMLElement>("[data-panel-meta]")!;
  const activity = root.querySelector<HTMLElement>("[data-panel-activity]")!;
  const destination = root.querySelector<HTMLElement>("[data-panel-destination]")!;
  const needs = root.querySelector<HTMLElement>("[data-panel-needs]")!;
  const energy = root.querySelector<HTMLElement>("[data-panel-energy]")!;
  const thought = root.querySelector<HTMLElement>("[data-panel-thought]")!;
  const group = root.querySelector<HTMLUListElement>("[data-panel-group]")!;
  const traits = root.querySelector<HTMLUListElement>("[data-panel-traits]")!;
  const memories = root.querySelector<HTMLUListElement>("[data-panel-memories]")!;
  const closeButton = root.querySelector<HTMLButtonElement>("[data-testid='guest-panel-close']")!;
  let detail: GuestDetail | undefined;
  let units: Units = "imperial";
  let anchor: GuestPanelAnchor | null = null;
  let positioned = false;
  let lastGuestId: string | undefined;

  const closeWithoutNotification = (): void => {
    root.hidden = true;
    root.removeAttribute("data-guest-id");
    detail = undefined;
  };
  const closeFromControl = (): void => callbacks.dispatch({ type: "guest-close" });
  closeButton.addEventListener("click", closeFromControl);
  const onKeydown = (event: KeyboardEvent): void => {
    if (event.key === "Escape" && !root.hidden) {
      event.preventDefault();
      closeFromControl();
    }
  };
  window.addEventListener("keydown", onKeydown);

  function render(current: GuestDetail): void {
    root.dataset.guestId = current.id;
    name.textContent = current.name;
    eyebrow.textContent = `${AGE_LABELS[current.age]} · ${capitalize(current.groupKind)}`;
    meta.replaceChildren(
      textSpan(`Visit ${current.visitNumber}`),
      textSpan(`Spent $${current.moneySpent}`),
      textSpan(`$${current.lifetimeMoneySpent} all visits`),
    );
    activity.textContent = activityLabel(current.activity);
    destination.textContent = current.activity.destination
      ? `Near tile ${current.activity.destination.x}, ${current.activity.destination.y}`
      : current.activity.placeId === null ? "" : `At stop ${current.activity.placeId + 1}`;
    needs.replaceChildren(...NEEDS.map(([label, key]) => needMeter(label, current.needs[key])));
    energy.textContent = `Energy ${current.energy}% · Cheer ${current.happiness}%`;
    thought.textContent = thoughtLabel(current.thought, current.thoughtText);
    group.replaceChildren(...current.groupMembers.map((member) => {
      const item = document.createElement("li");
      const label = relationshipLabel(member.relationship);
      if (member.id === current.id) {
        const self = document.createElement("span");
        self.className = "guest-panel__member";
        self.setAttribute("aria-current", "true");
        self.append(textSpan(member.name), smallText(label));
        item.append(self);
      } else {
        const button = document.createElement("button");
        button.className = "guest-panel__member";
        button.type = "button";
        button.dataset.guestId = member.id;
        button.append(textSpan(member.name), smallText(label));
        button.addEventListener("click", () => callbacks.dispatch({ type: "guest-select", guestId: member.id }));
        item.append(button);
      }
      return item;
    }));
    const favoriteTypes = current.traits.favouriteRideTypes.length
      ? `Favourite ride styles: ${current.traits.favouriteRideTypes.map((type) => `#${type + 1}`).join(", ")}`
      : "Still finding a favourite ride";
    const traitLines = [
      ...traitSummary(current.traits.intensity, current.traits.themingCare),
      `Nausea tolerance: ${current.traits.nauseaTolerance}%`,
      favoriteTypes,
      `Spending: ${current.traits.spending.toLowerCase()}`,
      `Height: ${units === "metric" ? `${current.traits.heightCm} cm` : formatLength(current.traits.heightCm / 100, units)}`,
      `Theme preference: ${current.traits.preferredTheme + 1}`,
    ];
    traits.replaceChildren(...traitLines.map(listItem));
    memories.replaceChildren(...[...current.memories].reverse().map((memory) => listItem(memoryLabel(memory))));
    root.hidden = false;
    placeBesideGuest();
  }

  function placeBesideGuest(): void {
    if (root.hidden || !anchor) return;
    const bounds = canvas.getBoundingClientRect();
    const scaleX = bounds.width / canvas.width;
    const scaleY = bounds.height / canvas.height;
    const guestX = clamp(bounds.left + anchor.x * scaleX, bounds.left + EDGE, bounds.right - EDGE);
    const guestY = clamp(bounds.top + anchor.y * scaleY, bounds.top + EDGE, bounds.bottom - EDGE);
    if (positioned && canKeepGuestPanelPosition(bounds, root.getBoundingClientRect(), { x: guestX, y: guestY })) return;
    const options = ([
      { side: "right", availableWidth: bounds.right - guestX - GAP - EDGE, availableHeight: bounds.height - EDGE * 2, score: 0 },
      { side: "left", availableWidth: guestX - bounds.left - GAP - EDGE, availableHeight: bounds.height - EDGE * 2, score: 0 },
      { side: "below", availableWidth: bounds.width - EDGE * 2, availableHeight: bounds.bottom - guestY - GAP - EDGE, score: 0 },
      { side: "above", availableWidth: bounds.width - EDGE * 2, availableHeight: guestY - bounds.top - GAP - EDGE, score: 0 },
    ] as const).map((option): Placement => ({
      ...option,
      score: Math.min(MAX_PANEL_WIDTH, Math.max(0, option.availableWidth)) * Math.min(bounds.height * 0.82, Math.max(0, option.availableHeight)),
    }));
    const placement = [...options].sort((first, second) => second.score - first.score)[0];
    root.style.maxWidth = `${Math.max(160, Math.min(MAX_PANEL_WIDTH, placement.availableWidth))}px`;
    root.style.maxHeight = `${Math.max(140, Math.min(bounds.height * 0.82, placement.availableHeight))}px`;
    const panelBounds = root.getBoundingClientRect();
    const width = panelBounds.width;
    const height = panelBounds.height;
    const left = placement.side === "right" ? guestX + GAP
      : placement.side === "left" ? guestX - GAP - width
        : clamp(guestX - width / 2, bounds.left + EDGE, bounds.right - width - EDGE);
    const top = placement.side === "below" ? guestY + GAP
      : placement.side === "above" ? guestY - GAP - height
        : clamp(guestY - height / 2, bounds.top + EDGE, bounds.bottom - height - EDGE);
    root.style.left = `${clamp(left, bounds.left + EDGE, bounds.right - width - EDGE)}px`;
    root.style.top = `${clamp(top, bounds.top + EDGE, bounds.bottom - height - EDGE)}px`;
    positioned = true;
  }

  new ResizeObserver(placeBesideGuest).observe(canvas);
  window.addEventListener("resize", placeBesideGuest);

  return {
    setUnits(value) {
      units = value;
      if (detail) render(detail);
    },
    update(next) {
      if (root.hidden && next.id !== lastGuestId) positioned = false;
      detail = next;
      lastGuestId = next.id;
      render(next);
    },
    setAnchor(next) {
      anchor = next;
      placeBesideGuest();
    },
    close() {
      closeWithoutNotification();
    },
  };
}

function needMeter(label: string, value: number): HTMLElement {
  if (!Number.isFinite(value) || value < 0 || value > 100) throw new RangeError(`Invalid ${label.toLowerCase()} need`);
  const wrapper = document.createElement("label");
  wrapper.className = "guest-panel__need";
  const name = document.createElement("span");
  name.textContent = label;
  const percentage = document.createElement("span");
  percentage.textContent = `${value}%`;
  const meter = document.createElement("progress");
  meter.max = 100;
  meter.value = value;
  meter.setAttribute("aria-label", `${label}: ${value}%`);
  wrapper.append(name, percentage, meter);
  return wrapper;
}

function listItem(value: string): HTMLLIElement {
  const item = document.createElement("li");
  item.textContent = value;
  return item;
}

function textSpan(value: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.textContent = value;
  return span;
}

function smallText(value: string): HTMLElement {
  const small = document.createElement("small");
  small.textContent = value;
  return small;
}

function capitalize(value: string): string {
  return `${value.slice(0, 1).toUpperCase()}${value.slice(1)}`;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(value, maximum));
}
