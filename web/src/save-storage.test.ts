import { describe, expect, it } from "vitest";
import {
  createParkSaveFile,
  createParkSaveStorage,
  readParkSaveFile,
  SaveFormatError,
  SaveStorageError,
  type ParkSaveRecordDatabase,
} from "./save-storage";

class MemorySaveDatabase implements ParkSaveRecordDatabase {
  private bytes: ArrayBuffer | undefined;

  async read(): Promise<ArrayBuffer | undefined> {
    return this.bytes;
  }

  async write(bytes: ArrayBuffer): Promise<void> {
    this.bytes = bytes;
  }
}

describe("park save storage and files", () => {
  it("round-trips opaque bytes through an exported file", async () => {
    const payload = Uint8Array.of(0, 255, 12, 128, 3);
    const file = createParkSaveFile(payload);

    expect(file.type).toBe("application/vnd.theme-park-sim.save");
    await expect(readParkSaveFile(file)).resolves.toEqual(payload);
  });

  it("rejects files with an unsupported container version without changing the file", async () => {
    const file = createParkSaveFile(Uint8Array.of(1, 2, 3));
    const original = await file.arrayBuffer();
    const oldFileBytes = new Uint8Array(original.slice(0));
    oldFileBytes[6] = 0;
    oldFileBytes[7] = 0;
    const oldFile = new Blob([oldFileBytes]);

    await expect(readParkSaveFile(oldFile)).rejects.toMatchObject({
      name: "SaveFormatError",
      code: "unsupported-version",
      version: 0,
    });
    expect(new Uint8Array(await oldFile.arrayBuffer())).toEqual(oldFileBytes);
  });

  it("rejects malformed and truncated files with a format error", async () => {
    await expect(readParkSaveFile(new Blob(["not a save"]))).rejects.toBeInstanceOf(SaveFormatError);
    await expect(readParkSaveFile(new Blob([Uint8Array.of(0x54, 0x50, 0x53)]))).rejects.toBeInstanceOf(SaveFormatError);
  });

  it("stores a copy of bytes and returns null when browser storage has no saved park", async () => {
    const database = new MemorySaveDatabase();
    const storage = createParkSaveStorage(database);
    const payload = Uint8Array.of(3, 5, 8);

    await expect(storage.load()).resolves.toBeNull();
    await storage.save(payload);
    payload[0] = 99;

    await expect(storage.load()).resolves.toEqual(Uint8Array.of(3, 5, 8));
  });

  it("reports a full browser storage quota clearly", async () => {
    const storage = createParkSaveStorage({
      async read() { return undefined; },
      async write() { throw new DOMException("Storage is full", "QuotaExceededError"); },
    });

    await expect(storage.save(Uint8Array.of(1))).rejects.toMatchObject({
      name: "SaveStorageError",
      code: "quota",
    } satisfies Partial<SaveStorageError>);
  });
});
