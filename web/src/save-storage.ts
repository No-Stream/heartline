const DATABASE_NAME = "theme-park-sim-saves";
const DATABASE_VERSION = 2;
const SAVE_STORE_NAME = "saves";
const SAVE_RECORD_KEY = "current-park";
const AUTOSAVE_METADATA_STORE_NAME = "autosaves";
const AUTOSAVE_BYTES_STORE_NAME = "autosave-bytes";
const SAVE_FILE_MAGIC = Uint8Array.of(0x54, 0x50, 0x53, 0x41, 0x56, 0x45);
const SAVE_FILE_VERSION = 1;
const SAVE_FILE_HEADER_LENGTH = SAVE_FILE_MAGIC.length + 2;
const SAVE_FILE_TYPE = "application/vnd.theme-park-sim.save";

export type SaveStorageErrorCode = "unavailable" | "quota" | "read" | "write";

export class SaveStorageError extends Error {
  constructor(readonly code: SaveStorageErrorCode, cause: unknown) {
    super(storageErrorMessage(code), { cause });
    this.name = "SaveStorageError";
  }
}

export type SaveFormatErrorCode = "invalid-file" | "unsupported-version";

export class SaveFormatError extends Error {
  constructor(readonly code: SaveFormatErrorCode, readonly version?: number) {
    super(formatErrorMessage(code, version));
    this.name = "SaveFormatError";
  }
}

export interface ParkSaveRecordDatabase {
  read(): Promise<ArrayBuffer | undefined>;
  write(bytes: ArrayBuffer): Promise<void>;
}

export interface ParkSaveStorage {
  load(): Promise<Uint8Array | null>;
  save(bytes: Uint8Array): Promise<void>;
}

export interface AutosaveSummary {
  id: string;
  savedAt: number;
  parkAgeTicks: number;
}

export interface AutosaveRecordDatabase {
  readAll(): Promise<AutosaveSummary[]>;
  read(id: string): Promise<ArrayBuffer | undefined>;
  write(summary: AutosaveSummary, bytes: ArrayBuffer, maxSlots: number): Promise<number>;
  delete(id: string): Promise<void>;
}

export interface AutosaveWriteResult {
  summary: AutosaveSummary;
  removedCount: number;
  quotaEvictedCount: number;
}

export interface AutosaveStorage {
  list(): Promise<AutosaveSummary[]>;
  load(id: string): Promise<Uint8Array | null>;
  delete(id: string): Promise<void>;
  save(bytes: Uint8Array, parkAgeTicks: number, maxSlots: number): Promise<AutosaveWriteResult>;
}

export class AutosaveStorageError extends Error {
  constructor(readonly code: SaveStorageErrorCode, readonly removedCount: number, cause: unknown, readonly quotaEvictedCount = 0) {
    super(autosaveStorageErrorMessage(code, removedCount), { cause });
    this.name = "AutosaveStorageError";
  }
}

export function createParkSaveStorage(database: ParkSaveRecordDatabase): ParkSaveStorage {
  return {
    async load(): Promise<Uint8Array | null> {
      let storedBytes: ArrayBuffer | undefined;
      try {
        storedBytes = await database.read();
      } catch (error) {
        throw asStorageError("read", error);
      }
      return storedBytes === undefined ? null : new Uint8Array(storedBytes.slice(0));
    },
    async save(bytes: Uint8Array): Promise<void> {
      assertSaveBytes(bytes);
      try {
        await database.write(new Uint8Array(bytes).buffer);
      } catch (error) {
        throw asStorageError("write", error);
      }
    },
  };
}

export function createAutosaveStorage(database: AutosaveRecordDatabase, now: () => number = Date.now): AutosaveStorage {
  return {
    async list(): Promise<AutosaveSummary[]> {
      return readAutosaves(database, 0);
    },
    async load(id: string): Promise<Uint8Array | null> {
      try {
        const storedBytes = await database.read(id);
        return storedBytes === undefined ? null : new Uint8Array(storedBytes.slice(0));
      } catch (error) {
        throw asAutosaveStorageError("read", error, 0);
      }
    },
    async delete(id: string): Promise<void> {
      await deleteAutosaveRecord(database, id, 0);
    },
    async save(bytes: Uint8Array, parkAgeTicks: number, maxSlots: number): Promise<AutosaveWriteResult> {
      assertSaveBytes(bytes);
      assertAutosaveMetadata(parkAgeTicks, maxSlots);

      let quotaEvictedCount = 0;
      const savedAt = now();
      const summary: AutosaveSummary = { id: `${savedAt}-${crypto.randomUUID()}`, savedAt, parkAgeTicks };
      const ownedBytes = new Uint8Array(bytes).buffer;
      while (true) {
        try {
          const rotatedCount = await database.write(summary, ownedBytes, maxSlots);
          return { summary, removedCount: rotatedCount + quotaEvictedCount, quotaEvictedCount };
        } catch (error) {
          const storageError = asStorageError("write", error);
          if (storageError.code !== "quota") throw new AutosaveStorageError(storageError.code, quotaEvictedCount, error, quotaEvictedCount);
          const autosaves = await readAutosaves(database, quotaEvictedCount);
          const oldest = autosaves.at(-1);
          if (!oldest) throw new AutosaveStorageError("quota", quotaEvictedCount, error, quotaEvictedCount);
          await deleteAutosaveRecord(database, oldest.id, quotaEvictedCount);
          quotaEvictedCount += 1;
        }
      }
    },
  };
}

/** Save bytes in IndexedDB, which supports parks much larger than localStorage. */
export function saveParkSave(bytes: Uint8Array): Promise<void> {
  return indexedDbStorage.save(bytes);
}

/** Return null when no save exists, including after browser storage was cleared. */
export function loadParkSave(): Promise<Uint8Array | null> {
  return indexedDbStorage.load();
}

export function listAutosaves(): Promise<AutosaveSummary[]> {
  return indexedDbAutosaveStorage.list();
}

export function loadAutosave(id: string): Promise<Uint8Array | null> {
  return indexedDbAutosaveStorage.load(id);
}

export function deleteAutosave(id: string): Promise<void> {
  return indexedDbAutosaveStorage.delete(id);
}

export function saveAutosave(bytes: Uint8Array, parkAgeTicks: number, maxSlots: number): Promise<AutosaveWriteResult> {
  return indexedDbAutosaveStorage.save(bytes, parkAgeTicks, maxSlots);
}

/** Wrap opaque Rust save bytes in the versioned browser file container. */
export function createParkSaveFile(bytes: Uint8Array): Blob {
  assertSaveBytes(bytes);
  const header = new Uint8Array(SAVE_FILE_HEADER_LENGTH);
  header.set(SAVE_FILE_MAGIC);
  new DataView(header.buffer).setUint16(SAVE_FILE_MAGIC.length, SAVE_FILE_VERSION, true);
  return new Blob([header, new Uint8Array(bytes)], { type: SAVE_FILE_TYPE });
}

/** Validate the browser file container and return the opaque Rust payload. */
export async function readParkSaveFile(file: Blob): Promise<Uint8Array> {
  if (file.size < SAVE_FILE_HEADER_LENGTH + 1) throw new SaveFormatError("invalid-file");

  const bytes = new Uint8Array(await file.arrayBuffer());
  if (!hasSaveFileMagic(bytes)) throw new SaveFormatError("invalid-file");

  const version = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    .getUint16(SAVE_FILE_MAGIC.length, true);
  if (version !== SAVE_FILE_VERSION) throw new SaveFormatError("unsupported-version", version);

  const payload = bytes.slice(SAVE_FILE_HEADER_LENGTH);
  assertSaveBytes(payload);
  return payload;
}

const indexedDbRecordDatabase: ParkSaveRecordDatabase = {
  async read(): Promise<ArrayBuffer | undefined> {
    return runSaveTransaction("readonly", (transaction, complete) => {
      const request = transaction.objectStore(SAVE_STORE_NAME).get(SAVE_RECORD_KEY) as IDBRequest<ArrayBuffer | undefined>;
      request.onsuccess = () => complete(request.result);
    });
  },
  async write(bytes: ArrayBuffer): Promise<void> {
    await runSaveTransaction("readwrite", (transaction, complete) => {
      const request = transaction.objectStore(SAVE_STORE_NAME).put(bytes, SAVE_RECORD_KEY);
      request.onsuccess = () => complete(undefined);
    });
  },
};

const indexedDbAutosaveRecordDatabase: AutosaveRecordDatabase = {
  async readAll(): Promise<AutosaveSummary[]> {
    return runSaveTransaction("readonly", (transaction, complete) => {
      const request = transaction.objectStore(AUTOSAVE_METADATA_STORE_NAME).getAll() as IDBRequest<AutosaveSummary[]>;
      request.onsuccess = () => complete(request.result);
    }, [AUTOSAVE_METADATA_STORE_NAME]);
  },
  async read(id: string): Promise<ArrayBuffer | undefined> {
    return runSaveTransaction("readonly", (transaction, complete) => {
      const request = transaction.objectStore(AUTOSAVE_BYTES_STORE_NAME).get(id) as IDBRequest<ArrayBuffer | undefined>;
      request.onsuccess = () => complete(request.result);
    }, [AUTOSAVE_BYTES_STORE_NAME]);
  },
  async write(summary: AutosaveSummary, bytes: ArrayBuffer, maxSlots: number): Promise<number> {
    return runSaveTransaction("readwrite", (transaction, complete) => {
      const metadata = transaction.objectStore(AUTOSAVE_METADATA_STORE_NAME);
      const readRequest = metadata.getAll() as IDBRequest<AutosaveSummary[]>;
      readRequest.onsuccess = () => {
        const existing = readRequest.result.sort((left, right) => right.savedAt - left.savedAt || right.id.localeCompare(left.id));
        const toRotate = existing.slice(maxSlots - 1);
        for (const entry of toRotate) {
          metadata.delete(entry.id);
          transaction.objectStore(AUTOSAVE_BYTES_STORE_NAME).delete(entry.id);
        }
        metadata.put(summary);
        transaction.objectStore(AUTOSAVE_BYTES_STORE_NAME).put(bytes, summary.id);
        complete(toRotate.length);
      };
    }, [AUTOSAVE_METADATA_STORE_NAME, AUTOSAVE_BYTES_STORE_NAME]);
  },
  async delete(id: string): Promise<void> {
    await runSaveTransaction("readwrite", (transaction, complete) => {
      transaction.objectStore(AUTOSAVE_METADATA_STORE_NAME).delete(id);
      transaction.objectStore(AUTOSAVE_BYTES_STORE_NAME).delete(id);
      complete(undefined);
    }, [AUTOSAVE_METADATA_STORE_NAME, AUTOSAVE_BYTES_STORE_NAME]);
  },
};

const indexedDbStorage = createParkSaveStorage(indexedDbRecordDatabase);
const indexedDbAutosaveStorage = createAutosaveStorage(indexedDbAutosaveRecordDatabase);

async function runSaveTransaction<Result>(
  mode: IDBTransactionMode,
  start: (transaction: IDBTransaction, complete: (result: Result) => void) => void,
  storeNames: string[] = [SAVE_STORE_NAME],
): Promise<Result> {
  const database = await openSaveDatabase();
  return new Promise<Result>((resolve, reject) => {
    let settled = false;
    let result: Result;
    let transaction: IDBTransaction;

    const complete = (value: Result): void => { result = value; };
    const fail = (error: unknown): void => {
      if (settled) return;
      settled = true;
      database.close();
      reject(error);
    };

    try {
      transaction = database.transaction(storeNames, mode);
      transaction.oncomplete = () => {
        if (settled) return;
        settled = true;
        database.close();
        resolve(result);
      };
      transaction.onerror = () => fail(transaction.error ?? new Error("IndexedDB transaction failed"));
      transaction.onabort = () => fail(transaction.error ?? new Error("IndexedDB transaction was aborted"));
      start(transaction, complete);
    } catch (error) {
      fail(error);
    }
  });
}

function openSaveDatabase(): Promise<IDBDatabase> {
  if (typeof indexedDB === "undefined") {
    return Promise.reject(new SaveStorageError("unavailable", new Error("IndexedDB is unavailable")));
  }

  return new Promise<IDBDatabase>((resolve, reject) => {
    let request: IDBOpenDBRequest;
    try {
      request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    } catch (error) {
      reject(asOpenStorageError(error));
      return;
    }
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(SAVE_STORE_NAME)) database.createObjectStore(SAVE_STORE_NAME);
      if (!database.objectStoreNames.contains(AUTOSAVE_METADATA_STORE_NAME)) {
        database.createObjectStore(AUTOSAVE_METADATA_STORE_NAME, { keyPath: "id" });
      }
      if (!database.objectStoreNames.contains(AUTOSAVE_BYTES_STORE_NAME)) database.createObjectStore(AUTOSAVE_BYTES_STORE_NAME);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(asOpenStorageError(request.error ?? new Error("IndexedDB could not open")));
    request.onblocked = () => reject(new SaveStorageError("unavailable", new Error("IndexedDB upgrade was blocked")));
  });
}

function hasSaveFileMagic(bytes: Uint8Array): boolean {
  return SAVE_FILE_MAGIC.every((byte, index) => bytes[index] === byte);
}

function assertSaveBytes(bytes: Uint8Array): void {
  if (bytes.byteLength === 0) throw new SaveFormatError("invalid-file");
}

function assertAutosaveMetadata(parkAgeTicks: number, maxSlots: number): void {
  if (!Number.isSafeInteger(parkAgeTicks) || parkAgeTicks < 0) throw new RangeError("Autosave park age must be a non-negative whole number of ticks");
  if (!Number.isInteger(maxSlots) || maxSlots < 1 || maxSlots > 100) throw new RangeError("Autosave slots must be between 1 and 100");
}

async function readAutosaves(database: AutosaveRecordDatabase, removedCount: number): Promise<AutosaveSummary[]> {
  try {
    const summaries = await database.readAll();
    return summaries.sort((left, right) => right.savedAt - left.savedAt || right.id.localeCompare(left.id));
  } catch (error) {
    throw asAutosaveStorageError("read", error, removedCount);
  }
}

async function deleteAutosaveRecord(database: AutosaveRecordDatabase, id: string, removedCount: number): Promise<void> {
  try {
    await database.delete(id);
  } catch (error) {
    throw asAutosaveStorageError("write", error, removedCount);
  }
}

function asAutosaveStorageError(operation: "read" | "write", error: unknown, removedCount: number): AutosaveStorageError {
  const storageError = asStorageError(operation, error);
  return new AutosaveStorageError(storageError.code, removedCount, error);
}

function asStorageError(operation: "read" | "write", error: unknown): SaveStorageError {
  if (error instanceof SaveStorageError) return error;
  if (error instanceof Error && error.name === "QuotaExceededError") return new SaveStorageError("quota", error);
  if (error instanceof Error && error.name === "SecurityError") return new SaveStorageError("unavailable", error);
  return new SaveStorageError(operation, error);
}

function asOpenStorageError(error: unknown): SaveStorageError {
  if (error instanceof SaveStorageError) return error;
  if (error instanceof Error && error.name === "QuotaExceededError") return new SaveStorageError("quota", error);
  return new SaveStorageError("unavailable", error);
}

function storageErrorMessage(code: SaveStorageErrorCode): string {
  switch (code) {
    case "unavailable": return "Browser storage is unavailable. Export the park to a save file to keep it.";
    case "quota": return "Browser storage is full. Export the park to a save file before clearing storage.";
    case "read": return "The park could not be read from browser storage. An exported save file is still safe.";
    case "write": return "The park could not be saved to browser storage. Export the park to a save file to keep it.";
  }
}

function autosaveStorageErrorMessage(code: SaveStorageErrorCode, removedCount: number): string {
  const base = code === "quota"
    ? "Browser storage is full. The current autosave could not be saved."
    : storageErrorMessage(code);
  if (removedCount === 0) return base;
  const removed = ` Removed ${removedCount} older autosave${removedCount === 1 ? "" : "s"}.`;
  return code === "quota" ? `Browser storage is full.${removed} The current autosave could not be saved.` : `${base}${removed}`;
}

function formatErrorMessage(code: SaveFormatErrorCode, version: number | undefined): string {
  if (code === "unsupported-version") {
    return `This save file uses container version ${version}; this game cannot read it. Keep the file and open it with a compatible game version.`;
  }
  return "This is not a valid Theme Park Sim save file.";
}
