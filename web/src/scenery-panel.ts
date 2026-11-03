import { SCENERY_SETS, type SceneryKind } from "./scenery-tool";
import type { FacilityArt } from "./facility-panel";
import "./scenery-panel.css";

const names = (kind: string): string => kind.split("_").map((word) => word[0].toUpperCase() + word.slice(1)).join(" ");
const directional = new Set<SceneryKind>(["flower_bed", "bench", "lamp", "fence", "signpost", "balloon_cart"]);

export function createSceneryPanel(container: HTMLElement, art: FacilityArt, place: (kind: SceneryKind, rotation: number) => void, remove: () => void, cancel: () => void) {
  container.classList.add("tool-panel", "scenery-panel");
  container.innerHTML = `<header class="tool-panel__header"><div><p class="tool-panel__eyebrow">Decorate the park</p><h2>Scenery</h2></div></header>
    ${Object.entries(SCENERY_SETS).map(([theme, kinds]) => `<section class="tool-panel__section"><h3>${theme}</h3><div class="scenery-panel__items" role="group" aria-label="${theme} scenery">${kinds.map((kind) => `<button type="button" data-scenery-kind="${kind}" aria-pressed="${kind === "tree_oval"}"><span>${names(kind)}</span></button>`).join("")}</div></section>`).join("")}
    <div class="tool-panel__section"><div class="stepper" role="group" aria-label="Scenery direction"><span class="tool-panel__label">Direction</span><button type="button" data-turn="-1" aria-label="Turn scenery anticlockwise">−</button><span class="stepper__value" data-direction></span><button type="button" data-turn="1" aria-label="Turn scenery clockwise">+</button></div>
    <p class="tool-panel__hint">Choose scenery and a direction, then click a tile to place it.</p>
    <div class="scenery-panel__actions"><button type="button" class="tool-panel__action tool-panel__action--primary" data-place>Place scenery</button><button type="button" class="tool-panel__action" data-remove>Remove scenery</button><button type="button" class="tool-panel__action tool-panel__action--quiet" data-cancel>Cancel</button></div><p class="tool-panel__status" role="status" aria-live="polite"></p></div>`;
  let selected: SceneryKind = "tree_oval";
  let rotation = 0;
  const status = container.querySelector<HTMLElement>("[role=status]")!;
  const direction = container.querySelector<HTMLElement>("[data-direction]")!;
  function update(): void {
    direction.textContent = ["North", "East", "South", "West"][rotation];
    for (const button of container.querySelectorAll<HTMLButtonElement>("[data-scenery-kind]")) button.setAttribute("aria-pressed", String(button.dataset.sceneryKind === selected));
  }
  for (const button of container.querySelectorAll<HTMLButtonElement>("[data-scenery-kind]")) {
    const kind = button.dataset.sceneryKind as SceneryKind;
    const view = directional.has(kind) ? "rot0" : "any";
    const placement = art.atlas.frames[`scenery/${kind}/${view}/${kind === "fountain" ? "loop" : "static"}/0`]?.placements["1"];
    if (!placement) throw new Error(`Atlas is missing scenery ${kind}`);
    const canvas = document.createElement("canvas"); canvas.width = 48; canvas.height = 42;
    const context = canvas.getContext("2d"); if (!context) throw new Error("Canvas 2D context is unavailable");
    const scale = Math.min(canvas.width / placement.width, canvas.height / placement.height);
    context.drawImage(art.images["1"][placement.page], placement.x, placement.y, placement.width, placement.height, (canvas.width - placement.width * scale) / 2, (canvas.height - placement.height * scale) / 2, placement.width * scale, placement.height * scale);
    const image = new Image(48, 42); image.alt = ""; image.src = canvas.toDataURL(); button.prepend(image);
    button.addEventListener("click", () => { selected = kind; update(); });
  }
  for (const button of container.querySelectorAll<HTMLButtonElement>("[data-turn]")) button.addEventListener("click", () => { rotation = (rotation + Number(button.dataset.turn) + 4) % 4; update(); });
  function activate(): void { place(selected, rotation); }
  container.querySelector<HTMLButtonElement>("[data-place]")!.addEventListener("click", activate);
  container.querySelector<HTMLButtonElement>("[data-remove]")!.addEventListener("click", remove);
  container.querySelector<HTMLButtonElement>("[data-cancel]")!.addEventListener("click", cancel);
  update();
  return { message(text: string) { status.textContent = text; }, activate };
}
