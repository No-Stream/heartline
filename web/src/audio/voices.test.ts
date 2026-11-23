import { describe, expect, it } from "vitest";
import { allocateTrainVoices } from "./voices";

describe("train voice allocation", () => {
  it("assigns the loudest four and releases inaudible or absent trains", () => {
    const voices = allocateTrainVoices([], [0.8, 0.5, 0.7, 0.6, 0.4].map((audibility, train) => ({ train: `1:${train}`, audibility })));
    expect(voices.filter((voice) => voice !== null)).toHaveLength(4);
    expect(voices).not.toContain("1:4");
    expect(allocateTrainVoices(voices, [{ train: "1:0", audibility: 0.01 }])).toEqual([null, null, null, null]);
  });

  it("keeps an existing voice within ten percent of a challenger", () => {
    const initial = allocateTrainVoices([], [0.9, 0.8, 0.7, 0.5].map((audibility, train) => ({ train: `1:${train}`, audibility })));
    const close = allocateTrainVoices(initial, [0.9, 0.8, 0.7, 0.5, 0.54].map((audibility, train) => ({ train: `1:${train}`, audibility })));
    expect(close).toEqual(initial);
    const louder = allocateTrainVoices(initial, [0.9, 0.8, 0.7, 0.5, 0.6].map((audibility, train) => ({ train: `1:${train}`, audibility })));
    expect(louder).toContain("1:4");
    expect(louder).not.toContain("1:3");
  });

  it("retains voices for same-index trains on distinct coasters independently", () => {
    const voices = allocateTrainVoices([], [
      { train: "17:0", audibility: 0.9 },
      { train: "19:0", audibility: 0.8 },
    ]);

    expect(allocateTrainVoices(voices, [
      { train: "19:0", audibility: 0.8 },
      { train: "17:0", audibility: 0.9 },
    ])).toEqual(voices);
  });
});
