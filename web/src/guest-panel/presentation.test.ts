import { describe, expect, it } from "vitest";
import { memoryLabel, thoughtLabel } from "./presentation";
import type { GuestMemory, GuestThought } from "./protocol";

const eventKinds = ["nearMiss", "headchopper", "dueling", "tunnel", "terrainHugging"] as const;

describe("guest panel phrasing", () => {
  it("phrases every memory variant from the wasm bridge", () => {
    const memories: Array<{ memory: GuestMemory; phrase: string }> = [
      { memory: { kind: "ride", rideId: 3, enjoyment: 8, nausea: 0, tick: 1 }, phrase: "Ride 4" },
      { memory: { kind: "food", placeId: 2, tick: 2 }, phrase: "stall 3" },
      { memory: { kind: "wait", placeId: 4, durationTicks: 90, tick: 3 }, phrase: "Waited about 3 seconds" },
      ...eventKinds.map((eventKind) => ({
        memory: { kind: "proximity" as const, rideId: 3, eventKind, position: { x: 1, y: 2, z: 3 }, strength: 0.8, tick: 4 },
        phrase: ({ nearMiss: "near-miss", headchopper: "overhead", dueling: "another train", tunnel: "tunnel", terrainHugging: "ground" } as const)[eventKind],
      })),
    ];

    for (const { memory, phrase } of memories) expect(memoryLabel(memory)).toContain(phrase);
    const proximityMemory = memories[3].memory as Extract<GuestMemory, { kind: "proximity" }>;
    expect(memoryLabel({ ...proximityMemory, eventKind: "futureEvent" })).not.toContain("futureEvent");
  });

  it("phrases every thought variant from the wasm bridge", () => {
    const thoughts: Array<{ thought: GuestThought; text: string; phrase: string }> = [
      { thought: { kind: "exploring", data: null }, text: "There is so much to see here.", phrase: "so much to see" },
      { thought: { kind: "hungry", data: null }, text: "I could really go for a snack.", phrase: "snack" },
      { thought: { kind: "thirsty", data: null }, text: "I could use a cold drink.", phrase: "cold drink" },
      { thought: { kind: "needsRestroom", data: null }, text: "I need to find a restroom soon.", phrase: "restroom" },
      { thought: { kind: "nauseous", data: null }, text: "That ride left me feeling a bit wobbly.", phrase: "wobbly" },
      { thought: { kind: "tired", data: null }, text: "My feet could use a rest.", phrase: "feet could use a rest" },
      { thought: { kind: "unhappy", data: null }, text: "I could use something to cheer me up.", phrase: "cheer me up" },
      { thought: { kind: "enjoyedRide", data: 3 }, text: "That ride was a real treat.", phrase: "real treat" },
      { thought: { kind: "dislikedRide", data: 3 }, text: "That ride was a bit much for me.", phrase: "bit much" },
      ...eventKinds.map((eventKind) => ({
        thought: { kind: "proximity" as const, data: { rideId: 3, eventKind } }, text: "",
        phrase: ({ nearMiss: "near-miss", headchopper: "hat off", dueling: "alongside the other train", tunnel: "tunnel", terrainHugging: "ground" } as const)[eventKind],
      })),
    ];

    for (const { thought, text, phrase } of thoughts) expect(thoughtLabel(thought, text)).toContain(phrase);
    expect(thoughtLabel({ kind: "futureThought", data: 987654 } as unknown as GuestThought, "987654")).not.toContain("987654");
  });
});
