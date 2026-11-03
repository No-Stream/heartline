import type { Facility, FacilityKind } from "./facilities";

export function createFacilityPanel(container: HTMLElement, select: (kind: FacilityKind, rotation: number) => void, remove: () => void, cancel: () => void, setPrice: (id: number, price: number) => void): { message(text: string): void; update(facilities: readonly Facility[], revenue: string): void } {
  container.innerHTML = `<h2>Park facilities</h2>
    <label>Facility <select aria-label="Facility">
      <option value="entrance">Entrance</option><option value="food_stall">Food stall</option><option value="drink_stall">Drink stall</option><option value="restroom">Restroom</option><option value="carousel">Carousel</option><option value="ferris_wheel">Ferris wheel</option><option value="souvenir_shop">Souvenir shop</option>
    </select></label>
    <label>Direction <select aria-label="Direction"><option value="0">North</option><option value="1">East</option><option value="2">South</option><option value="3">West</option></select></label>
    <button type="button" data-place>Place facility</button><button type="button" data-remove>Remove facility</button><button type="button" data-cancel>Cancel</button><p>Choose a flat tile next to a path. The direction points towards the path.</p><p role="status"></p><p>Facility revenue: <output data-revenue>0</output></p><ul data-prices></ul>`;
  const kind = container.querySelector<HTMLSelectElement>('[aria-label="Facility"]')!;
  const direction = container.querySelector<HTMLSelectElement>('[aria-label="Direction"]')!;
  const message = container.querySelector<HTMLElement>('[role="status"]')!;
  container.querySelector("[data-place]")!.addEventListener("click", () => { select(kind.value as FacilityKind, Number(direction.value)); message.textContent = "Choose a tile for the facility."; });
  container.querySelector("[data-remove]")!.addEventListener("click", () => { remove(); message.textContent = "Choose a facility to remove."; });
  container.querySelector("[data-cancel]")!.addEventListener("click", () => { cancel(); message.textContent = "Facility selection cancelled."; });
  return {
    message(text) { message.textContent = text; },
    update(facilities, revenue) {
      container.querySelector<HTMLOutputElement>("[data-revenue]")!.value = revenue;
      const list = container.querySelector<HTMLUListElement>("[data-prices]")!;
      list.replaceChildren(...facilities.map((facility) => {
        const item = document.createElement("li");
        const label = document.createElement("label");
        label.textContent = `${facility.kind.replaceAll("_", " ")} ${facility.id} price `;
        const input = document.createElement("input");
        input.type = "number"; input.min = "0"; input.max = "65535"; input.step = "1"; input.value = String(facility.price);
        input.addEventListener("change", () => { if (input.validity.valid) setPrice(facility.id, Number(input.value)); });
        label.append(input); item.append(label); return item;
      }));
    },
  };
}
