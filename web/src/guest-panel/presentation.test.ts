import { describe, expect, it } from "vitest";
import { memoryLabel, thoughtLabel } from "./presentation";
import type { GuestMemory, GuestThought } from "./protocol";

const eventKinds = ["nearMiss", "headchopper", "dueling", "tunnel", "terrainHugging"] as const;

describe("guest panel phrasing", () => {
  it("phrases every memory variant from the wasm bridge", () => {
    const memories: Array<{ memory: GuestMemory; phrase: string }> = [
      { memory: { kind: "ride", rideId: 3, enjoyment: 8, nausea: 0, tick: 1 }, phrase: "Ride 4" },
      { memory: { kind: "food", placeId: 2, tick: 2 }, phrase: "stall 3" },
      {
        memory: { kind: "wait", placeId: 4, durationTicks: 90, perceivedDurationTicks: 90, tick: 3 },
        phrase: "Waited about 3 seconds",
      },
      {
        memory: { kind: "wait", placeId: 4, durationTicks: 150, perceivedDurationTicks: 60, tick: 4 },
        phrase: "Waited about 5 seconds for stop 5 (felt like 2 seconds)",
      },
      { memory: { kind: "scenery", theme: 0, enjoyment: 8, tick: 5 }, phrase: "Garden scenery (+8 enjoyment)" },
      { memory: { kind: "scenery", theme: 1, enjoyment: 4, tick: 6 }, phrase: "Woodland scenery (+4 enjoyment)" },
      ...eventKinds.map((eventKind) => ({
        memory: { kind: "proximity" as const, rideId: 3, eventKind, position: { x: 1, y: 2, z: 3 }, strength: 0.8, tick: 4 },
        phrase: ({ nearMiss: "near-miss", headchopper: "overhead", dueling: "another train", tunnel: "tunnel", terrainHugging: "ground" } as const)[eventKind],
      })),
    ];

    for (const { memory, phrase } of memories) expect(memoryLabel(memory)).toContain(phrase);
    expect(memoryLabel({ kind: "wait", placeId: 4, durationTicks: 90, perceivedDurationTicks: 90, tick: 3 }))
      .toBe("Waited about 3 seconds for stop 5");
    const proximityMemory = memories.find(({ memory }) => memory.kind === "proximity")?.memory;
    if (proximityMemory?.kind !== "proximity") throw new Error("Expected a proximity memory fixture");
    expect(memoryLabel({ ...proximityMemory, eventKind: "futureEvent" })).not.toContain("futureEvent");
    expect(memoryLabel({ kind: "scenery", theme: 99, enjoyment: 5, tick: 7 })).not.toContain("99");
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
      { thought: { kind: "enjoyedScenery", data: 0 }, text: "The scenery here is lovely.", phrase: "scenery here is lovely" },
      ...eventKinds.map((eventKind) => ({
        thought: { kind: "proximity" as const, data: { rideId: 3, eventKind } }, text: "",
        phrase: ({ nearMiss: "near-miss", headchopper: "hat off", dueling: "alongside the other train", tunnel: "tunnel", terrainHugging: "ground" } as const)[eventKind],
      })),
    ];

    for (const { thought, text, phrase } of thoughts) expect(thoughtLabel(thought, text)).toContain(phrase);
    expect(thoughtLabel({ kind: "futureThought", data: 987654 } as unknown as GuestThought, "987654")).not.toContain("987654");
  });
});
