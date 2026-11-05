import {
  AutosaveStorageError,
  deleteAutosave,
  saveAutosave,
  type AutosaveWriteResult,
} from "./save-storage";

export {
  deleteAutosave,
  listAutosaves,
  loadAutosave,
  saveAutosave as storeAutosave,
} from "./save-storage";
export type { AutosaveSummary } from "./save-storage";

export interface AutosaveSettings {
  intervalMinutes: number;
  maxSlots: number;
}

export interface AutosaveSnapshot {
  bytes: Uint8Array;
  parkAgeTicks: number;
}

export interface AutosaveSchedulerOptions {
  settings: AutosaveSettings;
  capture(): Promise<AutosaveSnapshot>;
  persist?(bytes: Uint8Array, parkAgeTicks: number, maxSlots: number): Promise<AutosaveWriteResult>;
  discard?(id: string): Promise<void>;
  message(text: string, openMenu?: boolean): void;
}

export interface AutosaveScheduler {
  update(nowMs: number, running: boolean, tickCount: number): Promise<void> | undefined;
  markChanged(): void;
  markSaved(version?: number): void;
  reset(): void;
  setSettings(settings: AutosaveSettings): void;
  version(): number;
  isDirty(): boolean;
}

const SETTINGS_STORAGE_KEY = "theme-park-sim.autosave";
const DEFAULT_SETTINGS: AutosaveSettings = { intervalMinutes: 5, maxSlots: 30 };
const MAX_INTERVAL_MINUTES = 1440;
const MAX_AUTOSAVE_SLOTS = 100;

export function getAutosaveSettings(): AutosaveSettings {
  const storage = preferenceStorage();
  if (!storage) return { ...DEFAULT_SETTINGS };
  let stored: string | null;
  try {
    stored = storage.getItem(SETTINGS_STORAGE_KEY);
  } catch (error) {
    if (error instanceof DOMException) return { ...DEFAULT_SETTINGS };
    throw error;
  }
  if (stored === null) return { ...DEFAULT_SETTINGS };

  let parsed: unknown;
  try {
    parsed = JSON.parse(stored);
  } catch (error) {
    if (error instanceof SyntaxError) return { ...DEFAULT_SETTINGS };
    throw error;
  }
  if (!isAutosaveSettings(parsed)) return { ...DEFAULT_SETTINGS };
  return { intervalMinutes: parsed.intervalMinutes, maxSlots: parsed.maxSlots };
}

export function saveAutosaveSettings(settings: AutosaveSettings): void {
  assertAutosaveSettings(settings);
  const storage = preferenceStorage();
  if (!storage) throw autosaveSettingsStorageError();
  try {
    storage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch (error) {
    if (error instanceof DOMException) throw autosaveSettingsStorageError(error);
    throw error;
  }
}

export function createAutosaveScheduler(options: AutosaveSchedulerOptions): AutosaveScheduler {
  assertAutosaveSettings(options.settings);
  const persist = options.persist ?? saveAutosave;
  const discard = options.discard ?? deleteAutosave;
  let settings = { ...options.settings };
  let dirty = false;
  let mutationVersion = 0;
  let parkGeneration = 0;
  let elapsedMs = 0;
  let lastUpdateMs: number | undefined;
  let lastTickCount: number | undefined;
  let lastUpdateWasRunning = false;
  let pendingSave: Promise<void> | undefined;

  function markChanged(): void {
    mutationVersion += 1;
    dirty = true;
  }

  function markSaved(savedVersion = mutationVersion): void {
    if (savedVersion !== mutationVersion) return;
    mutationVersion += 1;
    dirty = false;
    elapsedMs = 0;
  }

  function reset(): void {
    mutationVersion += 1;
    parkGeneration += 1;
    dirty = false;
    elapsedMs = 0;
    lastUpdateMs = undefined;
    lastTickCount = undefined;
    lastUpdateWasRunning = false;
  }

  function startSave(): Promise<void> {
    const capturedVersion = mutationVersion;
    const capturedParkGeneration = parkGeneration;
    elapsedMs = 0;
    const pending = Promise.resolve()
      .then(options.capture)
      .then(async (snapshot) => {
        if (parkGeneration !== capturedParkGeneration) return;
        const result = await persist(snapshot.bytes, snapshot.parkAgeTicks, settings.maxSlots);
        if (parkGeneration !== capturedParkGeneration) {
          await discard(result.summary.id);
          return;
        }
        options.message(autosaveSuccessMessage(result.removedCount, result.quotaEvictedCount), result.quotaEvictedCount > 0);
        if (mutationVersion === capturedVersion) dirty = false;
      })
      .catch((error: unknown) => {
        if (parkGeneration !== capturedParkGeneration) return;
        options.message(autosaveFailureMessage(error));
      })
      .finally(() => {
        elapsedMs = 0;
        pendingSave = undefined;
      });
    pendingSave = pending;
    return pending;
  }

  return {
    update(nowMs: number, running: boolean, tickCount: number): Promise<void> | undefined {
      if (!Number.isFinite(nowMs) || nowMs < 0 || !Number.isSafeInteger(tickCount) || tickCount < 0) {
        throw new RangeError("Invalid autosave scheduler time or tick count");
      }

      if (lastUpdateMs !== undefined) {
        const elapsedSinceUpdate = nowMs - lastUpdateMs;
        if (elapsedSinceUpdate < 0) throw new RangeError("Autosave scheduler time must be monotonic");
        if (dirty && running && lastUpdateWasRunning) elapsedMs += elapsedSinceUpdate;
      }

      if (lastTickCount !== undefined && tickCount > lastTickCount) markChanged();
      lastUpdateMs = nowMs;
      lastUpdateWasRunning = running;
      lastTickCount = tickCount;

      if (pendingSave) return pendingSave;
      if (!dirty || elapsedMs < settings.intervalMinutes * 60_000) return undefined;
      return startSave();
    },
    markChanged,
    markSaved,
    reset,
    setSettings(nextSettings): void {
      assertAutosaveSettings(nextSettings);
      settings = { ...nextSettings };
    },
    version(): number { return mutationVersion; },
    isDirty(): boolean { return dirty; },
  };
}

function preferenceStorage(): Pick<Storage, "getItem" | "setItem"> | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch (error) {
    if (error instanceof DOMException) return null;
    throw error;
  }
}

function autosaveSettingsStorageError(cause?: DOMException): Error {
  return new Error("Autosave settings could not be persisted in browser storage.", { cause });
}

function isAutosaveSettings(value: unknown): value is AutosaveSettings {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const settings = value as Record<string, unknown>;
  return Number.isInteger(settings.intervalMinutes) &&
    typeof settings.intervalMinutes === "number" &&
    settings.intervalMinutes >= 1 && settings.intervalMinutes <= MAX_INTERVAL_MINUTES &&
    Number.isInteger(settings.maxSlots) &&
    typeof settings.maxSlots === "number" &&
    settings.maxSlots >= 1 && settings.maxSlots <= MAX_AUTOSAVE_SLOTS;
}

function assertAutosaveSettings(settings: AutosaveSettings): void {
  if (!isAutosaveSettings(settings)) throw new RangeError("Autosave settings are outside their supported ranges");
}

function autosaveSuccessMessage(removedCount: number, quotaEvictedCount: number): string {
  if (removedCount === 0) return "Autosave saved.";
  if (quotaEvictedCount > 0) {
    return `Autosave saved after browser storage filled. Removed ${quotaEvictedCount} older autosave${quotaEvictedCount === 1 ? "" : "s"}.`;
  }
  return `Autosave saved. Rotated out ${removedCount} oldest autosave${removedCount === 1 ? "" : "s"}.`;
}

function autosaveFailureMessage(error: unknown): string {
  if (error instanceof AutosaveStorageError && error.code === "quota") return error.message;
  return `Autosave failed: ${error instanceof Error ? error.message : String(error)}`;
}
