import { describe, expect, it } from "vitest";
import { parseStoredSound } from "./sound-setting";

describe("parseStoredSound", () => {
  it("accepts a valid persisted sound preference", () => {
    expect(parseStoredSound('{"volume":0.7,"muted":false}')).toEqual({ volume: 0.7, muted: false });
  });

  it.each([
    "not json",
    "null",
    "[]",
    "{}",
    '{"volume":0.7}',
    '{"volume":"0.7","muted":false}',
    '{"volume":-0.1,"muted":false}',
    '{"volume":1.1,"muted":false}',
    '{"volume":0.7,"muted":"false"}',
  ])("rejects malformed preference %s", (stored) => {
    expect(parseStoredSound(stored)).toBeUndefined();
  });
});
