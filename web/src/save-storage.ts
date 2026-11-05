const DATABASE_NAME = "theme-park-sim-saves";
const DATABASE_VERSION = 1;
const SAVE_STORE_NAME = "saves";
const SAVE_RECORD_KEY = "current-park";
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

/** Save bytes in IndexedDB, which supports parks much larger than localStorage. */
export function saveParkSave(bytes: Uint8Array): Promise<void> {
  return indexedDbStorage.save(bytes);
}

/** Return null when no save exists, including after browser storage was cleared. */
export function loadParkSave(): Promise<Uint8Array | null> {
  return indexedDbStorage.load();
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

const indexedDbStorage = createParkSaveStorage(indexedDbRecordDatabase);

async function runSaveTransaction<Result>(
  mode: IDBTransactionMode,
  start: (transaction: IDBTransaction, complete: (result: Result) => void) => void,
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
      transaction = database.transaction(SAVE_STORE_NAME, mode);
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

function formatErrorMessage(code: SaveFormatErrorCode, version: number | undefined): string {
  if (code === "unsupported-version") {
    return `This save file uses container version ${version}; this game cannot read it. Keep the file and open it with a compatible game version.`;
  }
  return "This is not a valid Theme Park Sim save file.";
}
