import { describe, expect, it } from "vitest";
import { rideFromPick } from "./picking";

const pick = (kind: string, id: string) => ({ kind, id, x: 0, y: 0, height: 0 });

describe("rideFromPick", () => {
  it("returns the coaster ID for track, supports and train parts", () => {
    expect(rideFromPick(pick("track", "track-42-3"))).toBe(42);
    expect(rideFromPick(pick("support", "support-42-12"))).toBe(42);
    expect(rideFromPick(pick("support", "footer-42-12"))).toBe(42);
    expect(rideFromPick(pick("train", "train-42-0"))).toBe(42);
  });

  it("preserves the reserved demo coaster ID and accepts the full u32 range", () => {
    expect(rideFromPick(pick("track", "track-4294967295-3"))).toBe(4294967295);
    expect(rideFromPick(pick("train", "train-4294967295-1"))).toBe(4294967295);
  });

  it("ignores ghosts, markers, guests, terrain and an empty pick", () => {
    expect(rideFromPick(pick("track", "preview-track-3"))).toBeNull();
    expect(rideFromPick(pick("track", "track-4294967296-3"))).toBeNull();
    expect(rideFromPick(pick("track", "track-42"))).toBeNull();
    expect(rideFromPick(pick("track", "support-42-3"))).toBeNull();
    expect(rideFromPick(pick("block-boundary", "block-boundary-0"))).toBeNull();
    expect(rideFromPick(pick("testing", "testing-4"))).toBeNull();
    expect(rideFromPick(pick("sprite", "guest-9"))).toBeNull();
    expect(rideFromPick({ kind: "terrain", x: 1, y: 2, height: 0 })).toBeNull();
    expect(rideFromPick(null)).toBeNull();
  });
});
