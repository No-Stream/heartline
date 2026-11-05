import { createParkSaveFile, loadParkSave, readParkSaveFile, saveParkSave } from "./save-storage";
import { deleteAutosave, getAutosaveSettings, listAutosaves, loadAutosave, saveAutosaveSettings, type AutosaveSettings } from "./autosave";
import "./park-saves.css";

export function createParkSaves(
  container: HTMLElement,
  save: () => Promise<Uint8Array>,
  load: (bytes: Uint8Array) => Promise<void>,
  onAutosaveSettingsChange: (settings: AutosaveSettings) => void,
): {
  message(text: string, openMenu?: boolean): void;
  refreshAutosaves(): Promise<void>;
} {
  container.className = "park-saves";
  container.innerHTML = `<button type="button" class="park-saves__toggle" aria-expanded="false">Park</button>
    <div class="park-saves__menu" hidden>
      <button type="button" data-save>Save park</button>
      <button type="button" data-load>Load park</button>
      <button type="button" data-export>Export park</button>
      <label>Import park file<input type="file" accept=".park,application/octet-stream" data-import></label>
      <section class="park-saves__autosaves" aria-labelledby="park-saves-autosaves-title">
        <h2 id="park-saves-autosaves-title">Autosaves</h2>
        <ul data-autosaves aria-live="polite"></ul>
      </section>
      <form class="park-saves__settings" data-autosave-settings>
        <label>Autosave interval (minutes)<input name="intervalMinutes" type="number" min="1" max="1440" step="1" required></label>
        <label>Autosave slots<input name="maxSlots" type="number" min="1" max="100" step="1" required></label>
        <button type="submit">Save autosave settings</button>
      </form>
    <p role="status" aria-live="polite"></p></div>`;
  const toggle = container.querySelector<HTMLButtonElement>(".park-saves__toggle")!;
  const menu = container.querySelector<HTMLElement>(".park-saves__menu")!;
  const status = container.querySelector<HTMLElement>("[role=status]")!;
  const file = container.querySelector<HTMLInputElement>("[data-import]")!;
  const autosaves = container.querySelector<HTMLUListElement>("[data-autosaves]")!;
  const settingsForm = container.querySelector<HTMLFormElement>("[data-autosave-settings]")!;
  const intervalInput = settingsForm.elements.namedItem("intervalMinutes") as HTMLInputElement;
  const slotsInput = settingsForm.elements.namedItem("maxSlots") as HTMLInputElement;
  let autosaveRefreshVersion = 0;

  const settings = getAutosaveSettings();
  intervalInput.value = String(settings.intervalMinutes);
  slotsInput.value = String(settings.maxSlots);

  function showMenu(): void {
    menu.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
  }
  function message(text: string, openMenu = true): void {
    status.textContent = text;
    if (openMenu) showMenu();
  }
  async function perform(action: () => Promise<void>): Promise<void> {
    try { await action(); } catch (error) { message(`Could not complete park operation: ${error instanceof Error ? error.message : String(error)}`); }
  }

  async function refreshAutosaves(): Promise<void> {
    const refreshVersion = ++autosaveRefreshVersion;
    const entries = await listAutosaves();
    if (refreshVersion !== autosaveRefreshVersion) return;
    autosaves.replaceChildren();
    if (entries.length === 0) {
      const empty = document.createElement("li");
      empty.className = "park-saves__empty";
      empty.textContent = "No autosaves yet.";
      autosaves.append(empty);
      return;
    }

    for (const entry of entries) {
      const item = document.createElement("li");
      item.dataset.autosaveId = entry.id;
      const details = document.createElement("p");
      const savedAt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(entry.savedAt);
      details.textContent = `Saved ${savedAt} · Park age ${formatParkAge(entry.parkAgeTicks)}`;
      const loadButton = document.createElement("button");
      loadButton.type = "button";
      loadButton.textContent = "Load autosave";
      loadButton.addEventListener("click", () => void perform(async () => {
        const bytes = await loadAutosave(entry.id);
        if (bytes === null) throw new Error("This autosave is no longer available.");
        await load(bytes);
        message("Autosave loaded.");
      }));
      const deleteButton = document.createElement("button");
      deleteButton.type = "button";
      deleteButton.textContent = "Delete autosave";
      deleteButton.addEventListener("click", () => void perform(async () => {
        await deleteAutosave(entry.id);
        await refreshAutosaves();
        message("Autosave deleted.");
      }));
      item.append(details, loadButton, deleteButton);
      autosaves.append(item);
    }
  }

  toggle.addEventListener("click", () => {
    menu.hidden = !menu.hidden;
    toggle.setAttribute("aria-expanded", String(!menu.hidden));
    if (!menu.hidden) void perform(refreshAutosaves);
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
  settingsForm.addEventListener("submit", (event: SubmitEvent) => {
    event.preventDefault();
    void perform(async () => {
      const intervalMinutes = Number(intervalInput.value);
      const maxSlots = Number(slotsInput.value);
      if (!Number.isInteger(intervalMinutes) || intervalMinutes < 1 || intervalMinutes > 1440) throw new RangeError("Autosave interval must be a whole number from 1 to 1440 minutes.");
      if (!Number.isInteger(maxSlots) || maxSlots < 1 || maxSlots > 100) throw new RangeError("Autosave slots must be a whole number from 1 to 100.");
      const updatedSettings: AutosaveSettings = { intervalMinutes, maxSlots };
      saveAutosaveSettings(updatedSettings);
      onAutosaveSettingsChange(updatedSettings);
      await refreshAutosaves();
      message("Autosave settings saved.");
    });
  });
  return { message, refreshAutosaves };
}

function formatParkAge(ticks: number): string {
  const totalSeconds = Math.floor(ticks / 30);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}
