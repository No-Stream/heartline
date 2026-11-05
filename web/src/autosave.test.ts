import { afterEach, describe, expect, it, vi } from "vitest";
import {
  createAutosaveScheduler,
  getAutosaveSettings,
  saveAutosaveSettings,
  type AutosaveSettings,
} from "./autosave";
import {
  AutosaveStorageError,
  createAutosaveStorage,
  createParkSaveStorage,
  type AutosaveRecordDatabase,
  type AutosaveSummary,
} from "./save-storage";

class MemoryAutosaveDatabase implements AutosaveRecordDatabase {
  readonly records = new Map<string, { summary: AutosaveSummary; bytes: ArrayBuffer }>();
  nextWriteError: unknown = undefined;

  constructor(private readonly quotaBytes = Number.POSITIVE_INFINITY) {}

  async readAll(): Promise<AutosaveSummary[]> {
    return [...this.records.values()].map(({ summary }) => ({ ...summary }));
  }

  async read(id: string): Promise<ArrayBuffer | undefined> {
    return this.records.get(id)?.bytes.slice(0);
  }

  async write(summary: AutosaveSummary, bytes: ArrayBuffer, maxSlots: number): Promise<number> {
    if (this.nextWriteError !== undefined) {
      const error = this.nextWriteError;
      this.nextWriteError = undefined;
      throw error;
    }
    const existing = [...this.records.values()].map(({ summary: entry }) => entry)
      .sort((left, right) => right.savedAt - left.savedAt || right.id.localeCompare(left.id));
    const toRotate = existing.slice(maxSlots - 1);
    const rotatedIds = new Set(toRotate.map((entry) => entry.id));
    const retainedBytes = [...this.records.values()]
      .filter(({ summary: entry }) => !rotatedIds.has(entry.id))
      .reduce((total, record) => total + record.bytes.byteLength, 0);
    if (retainedBytes + bytes.byteLength > this.quotaBytes) {
      throw new DOMException("Storage is full", "QuotaExceededError");
    }
    for (const id of rotatedIds) this.records.delete(id);
    this.records.set(summary.id, { summary: { ...summary }, bytes: bytes.slice(0) });
    return toRotate.length;
  }

  async delete(id: string): Promise<void> {
    this.records.delete(id);
  }
}

class MemorySettingsStorage {
  private readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("autosave storage", () => {
  it("stores independent bytes, lists newest first, and rotates to the configured slot count", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-05T00:00:00.000Z"));
    const database = new MemoryAutosaveDatabase();
    const storage = createAutosaveStorage(database);
    const firstBytes = Uint8Array.of(1, 2, 3);

    await storage.save(firstBytes, 30, 2);
    firstBytes[0] = 99;
    vi.advanceTimersByTime(1000);
    await storage.save(Uint8Array.of(4), 60, 2);
    vi.advanceTimersByTime(1000);
    const thirdSave = await storage.save(Uint8Array.of(5), 90, 2);

    const entries = await storage.list();
    expect(thirdSave.removedCount).toBe(1);
    expect(thirdSave.quotaEvictedCount).toBe(0);
    expect(entries).toHaveLength(2);
    expect(entries.map((entry) => entry.parkAgeTicks)).toEqual([90, 60]);
    expect(await storage.load(entries[1].id)).toEqual(Uint8Array.of(4));
    expect(await storage.load(entries[0].id)).toEqual(Uint8Array.of(5));
  });

  it("evicts only autosaves on quota errors and reports deletions if the new save still cannot fit", async () => {
    const database = new MemoryAutosaveDatabase(5);
    const storage = createAutosaveStorage(database);
    const manualSaveDatabase = {
      bytes: undefined as ArrayBuffer | undefined,
      async read(): Promise<ArrayBuffer | undefined> { return this.bytes; },
    async write(bytes: ArrayBuffer): Promise<void> { this.bytes = bytes.slice(0); },
    };
    const manualStorage = createParkSaveStorage(manualSaveDatabase);

    await manualStorage.save(Uint8Array.of(7, 8));
    await storage.save(Uint8Array.of(1, 2), 30, 10);
    await storage.save(Uint8Array.of(3, 4), 60, 10);
    const saved = await storage.save(Uint8Array.of(5, 6, 7, 8), 90, 10);

    expect(saved.removedCount).toBe(2);
    expect(saved.quotaEvictedCount).toBe(2);
    await expect(manualStorage.load()).resolves.toEqual(Uint8Array.of(7, 8));
    expect(await storage.load(saved.summary.id)).toEqual(Uint8Array.of(5, 6, 7, 8));

    const tooSmallStorage = createAutosaveStorage(new MemoryAutosaveDatabase(1));
    await tooSmallStorage.save(Uint8Array.of(1), 30, 10);
    await expect(tooSmallStorage.save(Uint8Array.of(2, 3), 60, 10)).rejects.toMatchObject({
      name: "AutosaveStorageError",
      code: "quota",
      removedCount: 1,
    } satisfies Partial<AutosaveStorageError>);
    await expect(tooSmallStorage.list()).resolves.toEqual([]);
  });

  it("keeps existing autosaves when a non-quota write fails", async () => {
    const database = new MemoryAutosaveDatabase();
    const storage = createAutosaveStorage(database);
    const existing = await storage.save(Uint8Array.of(1), 30, 10);
    database.nextWriteError = new Error("IndexedDB write failed");

    await expect(storage.save(Uint8Array.of(2), 60, 10)).rejects.toMatchObject({ code: "write", removedCount: 0 });
    await expect(storage.list()).resolves.toMatchObject([{ id: existing.summary.id }]);
  });
});

describe("autosave preferences", () => {
  it("loads defaults and persists validated interval and slot preferences", () => {
    const storage = new MemorySettingsStorage();
    vi.stubGlobal("localStorage", storage);

    expect(getAutosaveSettings()).toEqual({ intervalMinutes: 5, maxSlots: 30 });
    const settings: AutosaveSettings = { intervalMinutes: 12, maxSlots: 8 };
    saveAutosaveSettings(settings);
    expect(getAutosaveSettings()).toEqual(settings);
    expect(() => saveAutosaveSettings({ intervalMinutes: 0, maxSlots: 8 })).toThrow(RangeError);
    expect(() => saveAutosaveSettings({ intervalMinutes: 12, maxSlots: 101 })).toThrow(RangeError);
  });

  it("reports when browser storage cannot be accessed while saving settings", () => {
    const originalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() { throw new DOMException("Storage is blocked", "SecurityError"); },
    });

    try {
      expect(getAutosaveSettings()).toEqual({ intervalMinutes: 5, maxSlots: 30 });
      expect(() => saveAutosaveSettings({ intervalMinutes: 5, maxSlots: 30 })).toThrow("Autosave settings could not be persisted");
    } finally {
      if (originalDescriptor) Object.defineProperty(globalThis, "localStorage", originalDescriptor);
      else delete (globalThis as { localStorage?: Storage }).localStorage;
    }
  });
});

describe("autosave scheduling", () => {
  it("counts only monotonic unpaused time while dirty, detects tick changes, and awaits one save at the interval", async () => {
    const saved: { age: number; count: number }[] = [];
    const messages: string[] = [];
    const scheduler = createAutosaveScheduler({
      settings: { intervalMinutes: 1, maxSlots: 2 },
      async capture() { return { bytes: Uint8Array.of(1), parkAgeTicks: 120 }; },
      async persist(_bytes, age, count) { saved.push({ age, count }); return { removedCount: 2, quotaEvictedCount: 0, summary: { id: "saved", savedAt: 0, parkAgeTicks: age } }; },
      message(text) { messages.push(text); },
    });

    await scheduler.update(0, true, 0);
    await scheduler.update(60_000, false, 1);
    expect(scheduler.isDirty()).toBe(true);
    expect(saved).toEqual([]);
    await scheduler.update(120_000, true, 1);
    await scheduler.update(179_999, true, 1);
    expect(saved).toEqual([]);
    await scheduler.update(180_000, true, 1);

    expect(saved).toEqual([{ age: 120, count: 2 }]);
    expect(scheduler.isDirty()).toBe(false);
    expect(messages).toEqual(["Autosave saved. Rotated out 2 oldest autosaves."]);
  });

  it("keeps edits made while paused dirty when a worker snapshot is in flight", async () => {
    let finishCapture: ((snapshot: { bytes: Uint8Array; parkAgeTicks: number }) => void) | undefined;
    const messages: string[] = [];
    const persisted: number[] = [];
    const scheduler = createAutosaveScheduler({
      settings: { intervalMinutes: 1, maxSlots: 2 },
      capture: () => new Promise((resolve) => { finishCapture = resolve; }),
      async persist(_bytes, age) { persisted.push(age); return { removedCount: 0, quotaEvictedCount: 0, summary: { id: "saved", savedAt: 0, parkAgeTicks: age } }; },
      message(text) { messages.push(text); },
    });

    await scheduler.update(0, false, 0);
    scheduler.markChanged();
    await scheduler.update(60_000, false, 0);
    await scheduler.update(60_000, true, 0);
    const pendingSave = scheduler.update(120_000, true, 0);
    expect(pendingSave).toBeDefined();
    await Promise.resolve();
    expect(finishCapture).toBeDefined();
    scheduler.markChanged();
    finishCapture!({ bytes: Uint8Array.of(1), parkAgeTicks: 0 });
    await pendingSave;

    expect(scheduler.isDirty()).toBe(true);
    expect(messages).toEqual(["Autosave saved."]);
    expect(persisted).toEqual([0]);
    scheduler.reset();
    expect(scheduler.isDirty()).toBe(false);
  });

  it("keeps a change made during the storage write dirty after that older snapshot saves", async () => {
    let finishWrite: ((result: { summary: AutosaveSummary; removedCount: number; quotaEvictedCount: number }) => void) | undefined;
    const messages: string[] = [];
    const scheduler = createAutosaveScheduler({
      settings: { intervalMinutes: 1, maxSlots: 2 },
      async capture() { return { bytes: Uint8Array.of(1), parkAgeTicks: 30 }; },
      persist: () => new Promise((resolve) => { finishWrite = resolve; }),
      message(text, openMenu) { messages.push(`${text}|${String(openMenu)}`); },
    });

    await scheduler.update(0, true, 0);
    scheduler.markChanged();
    const pendingSave = scheduler.update(60_000, true, 0);
    await flushMicrotasks();
    expect(finishWrite).toBeDefined();
    scheduler.markChanged();
    finishWrite!({ summary: { id: "saved", savedAt: 0, parkAgeTicks: 30 }, removedCount: 0, quotaEvictedCount: 0 });
    await pendingSave;

    expect(scheduler.isDirty()).toBe(true);
    expect(messages).toEqual(["Autosave saved.|false"]);
  });

  it("deletes a completed write if a park load resets the scheduler during storage", async () => {
    let finishWrite: ((result: { summary: AutosaveSummary; removedCount: number; quotaEvictedCount: number }) => void) | undefined;
    const deletedIds: string[] = [];
    const messages: string[] = [];
    const scheduler = createAutosaveScheduler({
      settings: { intervalMinutes: 1, maxSlots: 2 },
      async capture() { return { bytes: Uint8Array.of(1), parkAgeTicks: 30 }; },
      persist: () => new Promise((resolve) => { finishWrite = resolve; }),
      async discard(id) { deletedIds.push(id); },
      message(text) { messages.push(text); },
    });

    await scheduler.update(0, true, 0);
    scheduler.markChanged();
    const pendingSave = scheduler.update(60_000, true, 0);
    await flushMicrotasks();
    expect(finishWrite).toBeDefined();
    scheduler.reset();
    finishWrite!({ summary: { id: "old-park", savedAt: 0, parkAgeTicks: 30 }, removedCount: 0, quotaEvictedCount: 0 });
    await pendingSave;

    expect(deletedIds).toEqual(["old-park"]);
    expect(messages).toEqual([]);
    expect(scheduler.isDirty()).toBe(false);
  });

  it("reports quota deletions when the newest autosave still cannot be stored", async () => {
    const messages: string[] = [];
    const scheduler = createAutosaveScheduler({
      settings: { intervalMinutes: 1, maxSlots: 2 },
      async capture() { return { bytes: Uint8Array.of(1), parkAgeTicks: 0 }; },
      async persist() { throw new AutosaveStorageError("quota", 3, new Error("Storage is full"), 2); },
      message(text) { messages.push(text); },
    });

    await scheduler.update(0, true, 0);
    scheduler.markChanged();
    await scheduler.update(60_000, true, 0);

    expect(messages).toEqual(["Browser storage is full. Removed 3 older autosaves. The current autosave could not be saved."]);
    expect(scheduler.isDirty()).toBe(true);
  });
});

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}
