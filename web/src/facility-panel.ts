import { facilityBuildings, type Facility, type FacilityKind } from "./facilities";
import type { InputAction } from "./input/actions";
import type { Atlas } from "./render/index";
import { trackStyle, type TrackStyle } from "./render/track/geometry";
import "./print-modern.css";
import "./facility-panel.css";

const facilityNames: Record<FacilityKind, string> = {
  entrance: "Entrance",
  food_stall: "Food stall",
  drink_stall: "Drink stall",
  restroom: "Restroom",
  carousel: "Carousel",
  ferris_wheel: "Ferris wheel",
  souvenir: "Souvenir shop",
};
const facilityKinds = Object.keys(facilityNames) as FacilityKind[];
const directionNames = ["North", "East", "South", "West"] as const;
const price = { minimum: 0, maximum: 65535 };

export interface FacilityArt { atlas: Atlas; images: Record<string, HTMLImageElement[]> }
export interface FacilityPanel { message(text: string): void; activate(): void; previewDirection(rotation: number | undefined): void; update(facilities: readonly Facility[], revenue: string): void }

/** Rides without atlas artwork get a small drawing in the same inks as their in-world geometry. */
function drawRide(context: CanvasRenderingContext2D, kind: "carousel" | "ferris_wheel", width: number, height: number, style: TrackStyle): void {
  const unit = Math.min(width, height) / 40;
  context.translate(width / 2, height / 2);
  context.scale(unit, unit);
  context.lineWidth = 1.4;
  context.lineJoin = "round";
  context.strokeStyle = style.ink;
  if (kind === "carousel") {
    context.fillStyle = style.wood;
    context.beginPath(); context.ellipse(0, 12, 15, 4.5, 0, 0, Math.PI * 2); context.fill(); context.stroke();
    for (const x of [-11, -4, 4, 11]) { context.beginPath(); context.moveTo(x, 12); context.lineTo(x, -3); context.stroke(); }
    for (let stripe = 0; stripe < 6; stripe++) {
      context.fillStyle = stripe % 2 ? style.train : style.steel;
      context.beginPath(); context.moveTo(0, -16); context.lineTo(-16 + stripe * 32 / 6, -3); context.lineTo(-16 + (stripe + 1) * 32 / 6, -3); context.closePath(); context.fill();
    }
    context.beginPath(); context.moveTo(0, -16); context.lineTo(-16, -3); context.lineTo(16, -3); context.closePath(); context.stroke();
  } else {
    context.strokeStyle = style.wood;
    context.lineWidth = 2.2;
    context.beginPath(); context.moveTo(-10, 17); context.lineTo(0, -3); context.lineTo(10, 17); context.stroke();
    context.strokeStyle = style.ink;
    context.lineWidth = 1;
    for (let spoke = 0; spoke < 8; spoke++) {
      const angle = spoke * Math.PI / 4;
      context.beginPath(); context.moveTo(0, -3); context.lineTo(Math.cos(angle) * 13, -3 + Math.sin(angle) * 13); context.stroke();
    }
    context.strokeStyle = style.steel;
    context.lineWidth = 2;
    context.beginPath(); context.arc(0, -3, 13, 0, Math.PI * 2); context.stroke();
    context.fillStyle = style.train;
    for (let car = 0; car < 8; car++) {
      const angle = car * Math.PI / 4 + Math.PI / 8;
      context.beginPath(); context.arc(Math.cos(angle) * 13, -3 + Math.sin(angle) * 13, 2, 0, Math.PI * 2); context.fill();
    }
  }
}

/**
 * A small drawing of a facility, cut from the 1x atlas page like the world renderer draws it. It is an image rather
 * than a canvas so the page keeps a single canvas, the world.
 */
function facilityThumbnail(art: FacilityArt, kind: FacilityKind, rotation: number, cssWidth: number, cssHeight: number): HTMLImageElement {
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(cssWidth * window.devicePixelRatio);
  canvas.height = Math.round(cssHeight * window.devicePixelRatio);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas 2D context is unavailable");
  context.imageSmoothingQuality = "high";
  const building = facilityBuildings[kind];
  if (building) drawBuilding(context, art, building, rotation, canvas.width, canvas.height);
  else drawRide(context, kind as "carousel" | "ferris_wheel", canvas.width, canvas.height, trackStyle(art.atlas));
  const image = new Image(cssWidth, cssHeight);
  image.src = canvas.toDataURL();
  image.alt = "";
  image.className = "facility-thumbnail";
  return image;
}

function drawBuilding(context: CanvasRenderingContext2D, art: FacilityArt, building: string, rotation: number, width: number, height: number): void {
  const key = `buildings/${building}/rot${rotation * 90}/static/0`;
  const placement = art.atlas.frames[key]?.placements["1"];
  if (!placement) throw new Error(`Atlas is missing ${key}`);
  const scale = Math.min(width / placement.width, height / placement.height);
  const drawnWidth = placement.width * scale, drawnHeight = placement.height * scale;
  context.drawImage(art.images["1"][placement.page], placement.x, placement.y, placement.width, placement.height, (width - drawnWidth) / 2, (height - drawnHeight) / 2, drawnWidth, drawnHeight);
}

export function createFacilityPanel(
  container: HTMLElement,
  art: FacilityArt,
  dispatch: (action: InputAction) => void,
): FacilityPanel {
  container.classList.add("tool-panel", "facility-panel");
  container.innerHTML = `<header class="tool-panel__header">
      <div><p class="tool-panel__eyebrow">Park services</p><h2>Facilities</h2></div>
      <p class="facility-panel__revenue"><span class="tool-panel__label">Revenue</span> <span data-revenue>0</span></p>
    </header>
    <div class="tool-panel__section facility-panel__picker" role="radiogroup" aria-label="Facility" data-kinds>${facilityKinds.map((kind) => `
      <button type="button" role="radio" class="facility-tile" data-kind="${kind}" aria-checked="${kind === "entrance"}" tabindex="${kind === "entrance" ? 0 : -1}"><span class="facility-tile__name">${facilityNames[kind]}</span></button>`).join("")}
    </div>
    <div class="tool-panel__section facility-panel__placement">
      <figure class="tool-panel__preview" data-preview><div class="facility-panel__preview-art" data-preview-art></div>
        <figcaption><span class="tool-panel__label">Next facility</span><span class="tool-panel__preview-name" data-preview-name></span></figcaption></figure>
      <div class="stepper" role="group" aria-label="Facility direction">
        <span class="tool-panel__label">Direction</span>
        <button type="button" data-direction-step="-1" aria-label="Turn anticlockwise">−</button>
        <span class="stepper__value" data-direction></span>
        <button type="button" data-direction-step="1" aria-label="Turn clockwise">+</button>
      </div>
      <p class="tool-panel__hint">Choose a flat tile next to a path. The direction points towards the path.</p>
      <div class="facility-panel__actions">
        <button type="button" class="tool-panel__action tool-panel__action--primary" data-place>Place facility</button>
        <button type="button" class="tool-panel__action" data-remove>Remove facility</button>
        <button type="button" class="tool-panel__action tool-panel__action--quiet" data-cancel>Cancel</button>
      </div>
      <p class="tool-panel__status facility-panel__status" role="status" aria-live="polite"></p>
    </div>
    <section class="tool-panel__section facility-panel__prices" aria-label="Prices">
      <h3>Prices</h3>
      <ul data-prices></ul>
    </section>`;
  const element = <T extends HTMLElement>(selector: string): T => {
    const found = container.querySelector<T>(selector);
    if (!found) throw new Error(`Facility panel is missing ${selector}`);
    return found;
  };
  const picker = element("[data-kinds]");
  const direction = element("[data-direction]");
  const message = element('[role="status"]');
  const prices = element<HTMLUListElement>("[data-prices]");
  // A plain span rather than an output, whose implicit live region would announce every sale.
  const revenue = element("[data-revenue]");
  let selectedKind: FacilityKind = "entrance";
  let rotation = 0;
  let hoveredRotation: number | undefined;
  let priceRowsKey = "";

  for (const tile of picker.querySelectorAll<HTMLButtonElement>("[data-kind]")) {
    tile.prepend(facilityThumbnail(art, tile.dataset.kind as FacilityKind, 0, 46, 40));
    tile.setAttribute("aria-label", facilityNames[tile.dataset.kind as FacilityKind]);
  }
  function renderSelection(): void {
    const previewRotation = hoveredRotation ?? rotation;
    for (const tile of picker.querySelectorAll<HTMLButtonElement>("[data-kind]")) {
      const checked = tile.dataset.kind === selectedKind;
      tile.setAttribute("aria-checked", String(checked));
      tile.tabIndex = checked ? 0 : -1;
    }
    element("[data-preview-art]").replaceChildren(facilityThumbnail(art, selectedKind, previewRotation, 64, 56));
    element("[data-preview-name]").textContent = `${facilityNames[selectedKind]}, facing ${directionNames[previewRotation].toLowerCase()}`;
    direction.textContent = directionNames[previewRotation];
  }
  function choose(kind: FacilityKind, focus: boolean): void {
    selectedKind = kind;
    hoveredRotation = undefined;
    renderSelection();
    if (focus) element(`[data-kind="${kind}"]`).focus();
  }
  picker.addEventListener("click", (event) => {
    const tile = (event.target as Element).closest<HTMLElement>("[data-kind]");
    if (tile) choose(tile.dataset.kind as FacilityKind, false);
  });
  picker.addEventListener("keydown", (event) => {
    const step = ({ ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 } as Record<string, number | undefined>)[event.key];
    if (!step) return;
    event.preventDefault();
    choose(facilityKinds[(facilityKinds.indexOf(selectedKind) + step + facilityKinds.length) % facilityKinds.length], true);
  });
  container.querySelectorAll<HTMLButtonElement>("[data-direction-step]").forEach((button) => button.addEventListener("click", () => {
    rotation = (rotation + Number(button.dataset.directionStep) + 4) % 4;
    renderSelection();
  }));
  function activate(): void {
    dispatch({ type: "facility-place-mode", kind: selectedKind, rotation });
    message.textContent = "Choose a tile for the facility.";
  }
  element("[data-place]").addEventListener("click", activate);
  element("[data-remove]").addEventListener("click", () => { dispatch({ type: "facility-remove-mode" }); message.textContent = "Choose a facility to remove."; });
  element("[data-cancel]").addEventListener("click", () => { dispatch({ type: "facility-cancel" }); message.textContent = "Facility selection cancelled."; });
  renderSelection();

  function priceRow(facility: Facility): HTMLLIElement {
    const name = `${facilityNames[facility.kind].toLowerCase()} ${facility.id}`;
    const item = document.createElement("li");
    item.className = "facility-price";
    item.dataset.facilityId = String(facility.id);
    item.innerHTML = `<span class="facility-price__name"></span><span class="facility-price__number">No. ${facility.id}</span>
      <div class="stepper stepper--compact" role="group">
        <button type="button" data-price-step="-1">−</button>
        <input type="number" min="${price.minimum}" max="${price.maximum}" step="1">
        <button type="button" data-price-step="1">+</button>
      </div>`;
    item.prepend(facilityThumbnail(art, facility.kind, facility.rotation, 30, 26));
    item.querySelector(".facility-price__name")!.textContent = facilityNames[facility.kind];
    item.querySelector(".stepper")!.setAttribute("aria-label", `Price of ${name}`);
    const input = item.querySelector("input")!;
    input.setAttribute("aria-label", `Price of ${name}`);
    const [lower, raise] = item.querySelectorAll<HTMLButtonElement>("[data-price-step]");
    lower.setAttribute("aria-label", `Lower the price of ${name}`);
    raise.setAttribute("aria-label", `Raise the price of ${name}`);
    const send = (value: number): void => {
      const clamped = Math.min(price.maximum, Math.max(price.minimum, value));
      input.value = String(clamped);
      dispatch({ type: "facility-price", id: facility.id, price: clamped });
    };
    input.addEventListener("change", () => { if (input.validity.valid && input.value !== "") send(Number(input.value)); });
    for (const button of [lower, raise]) button.addEventListener("click", () => send(Number(input.value) + Number(button.dataset.priceStep)));
    return item;
  }

  return {
    message(text) { message.textContent = text; },
    activate,
    previewDirection(rotation) { hoveredRotation = rotation; renderSelection(); },
    update(facilities, nextRevenue) {
      revenue.textContent = nextRevenue;
      const nextRowsKey = facilities.map(({ id, kind, rotation: facing }) => `${id}:${kind}:${facing}`).join("|");
      // Rebuild rows only when facilities come or go, so a stepper keeps focus while its price updates.
      if (nextRowsKey !== priceRowsKey) { priceRowsKey = nextRowsKey; prices.replaceChildren(...facilities.map(priceRow)); }
      for (const facility of facilities) {
        const input = prices.querySelector<HTMLInputElement>(`[data-facility-id="${facility.id}"] input`)!;
        if (document.activeElement !== input) input.value = String(facility.price);
      }
    },
  };
}
