import { describe, expect, it } from "vitest";
import { rideFromPick } from "./picking";

const pick = (kind: string, id: string) => ({ kind, id, x: 0, y: 0, height: 0 });

describe("rideFromPick", () => {
  it("maps the demo coaster's track, supports and train to the demo ride", () => {
    expect(rideFromPick(pick("track", "track-3"))).toBe("demo");
    expect(rideFromPick(pick("support", "support-12"))).toBe("demo");
    expect(rideFromPick(pick("train", "train-0"))).toBe("demo");
  });

  it("maps the player's coaster parts, which carry the construction prefix, to the player ride", () => {
    expect(rideFromPick(pick("track", "construction-track-3"))).toBe("player");
    expect(rideFromPick(pick("support", "construction-support-12"))).toBe("player");
    expect(rideFromPick(pick("train", "construction-train-1"))).toBe("player");
  });

  it("ignores ghosts, markers, guests, terrain and an empty pick", () => {
    expect(rideFromPick(pick("track", "preview-track-3"))).toBeNull();
    expect(rideFromPick(pick("block-boundary", "block-boundary-0"))).toBeNull();
    expect(rideFromPick(pick("testing", "testing-4"))).toBeNull();
    expect(rideFromPick(pick("sprite", "guest-9"))).toBeNull();
    expect(rideFromPick({ kind: "terrain", x: 1, y: 2, height: 0 })).toBeNull();
    expect(rideFromPick(null)).toBeNull();
  });
});
