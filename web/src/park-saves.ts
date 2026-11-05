import { createParkSaveFile, loadParkSave, readParkSaveFile, saveParkSave } from "./save-storage";
import "./park-saves.css";

export function createParkSaves(container: HTMLElement, save: () => Promise<Uint8Array>, load: (bytes: Uint8Array) => Promise<void>): { message(text: string): void } {
  container.className = "park-saves";
  container.innerHTML = `<button type="button" class="park-saves__toggle" aria-expanded="false">Park saves</button>
    <div class="park-saves__menu" hidden>
      <button type="button" data-save>Save park</button>
      <button type="button" data-load>Load park</button>
      <button type="button" data-export>Export park</button>
      <label>Import park file<input type="file" accept=".park,application/octet-stream" data-import></label>
    <p role="status" aria-live="polite"></p></div>`;
  const toggle = container.querySelector<HTMLButtonElement>(".park-saves__toggle")!;
  const menu = container.querySelector<HTMLElement>(".park-saves__menu")!;
  const status = container.querySelector<HTMLElement>("[role=status]")!;
  const file = container.querySelector<HTMLInputElement>("[data-import]")!;
  function message(text: string): void { status.textContent = text; menu.hidden = false; toggle.setAttribute("aria-expanded", "true"); }
  async function perform(action: () => Promise<void>): Promise<void> {
    try { await action(); } catch (error) { message(`Could not complete park operation: ${error instanceof Error ? error.message : String(error)}`); }
  }
  toggle.addEventListener("click", () => {
    menu.hidden = !menu.hidden;
    toggle.setAttribute("aria-expanded", String(!menu.hidden));
  });
  container.querySelector<HTMLButtonElement>("[data-save]")!.addEventListener("click", () => void perform(async () => {
    await saveParkSave(await save());
    message("Park saved in browser storage.");
  }));
  container.querySelector<HTMLButtonElement>("[data-load]")!.addEventListener("click", () => void perform(async () => {
    const saved = await loadParkSave();
    if (saved === null) throw new Error("No saved park was found. Browser storage may have been cleared; import an exported park file.");
    await load(saved);
    message("Park loaded.");
  }));
  container.querySelector<HTMLButtonElement>("[data-export]")!.addEventListener("click", () => void perform(async () => {
    const url = URL.createObjectURL(createParkSaveFile(await save()));
    const link = document.createElement("a");
    link.href = url;
    link.download = `park-${new Date().toISOString().replace(/[:.]/g, "-")}.park`;
    link.click();
    URL.revokeObjectURL(url);
    message("Park exported.");
  }));
  file.addEventListener("change", () => void perform(async () => {
    const selected = file.files?.[0];
    if (!selected) return;
    try { await load(await readParkSaveFile(selected)); } finally { file.value = ""; }
    message("Park imported.");
  }));
  return { message };
}
