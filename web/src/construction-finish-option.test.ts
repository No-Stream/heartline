import { describe, expect, it } from "vitest";
import { describeFinishOption } from "./construction";

const finishOptions = {
  previewedIndex: 1,
  options: [
    { rank: 1, pieces: ["OffsetLeft", "OneTileStraight"], arrivalSpeedMps: 8.66, brakeEnergyKj: 0, minimumForceHeadroomG: 1.05, minimumRatedMarginMps: null },
    { rank: 2, pieces: ["Brakes", "OneTileStraight"], arrivalSpeedMps: 3.7, brakeEnergyKj: 223.4, minimumForceHeadroomG: 0.91, minimumRatedMarginMps: null },
  ],
};

describe("describeFinishOption", () => {
  it("names the previewed option, its pieces, arrival speed and brake energy", () => {
    expect(describeFinishOption(finishOptions, "metric")).toBe(
      "Option 2 of 2: 2 pieces (Brakes, OneTileStraight). Arrives at 13 km/h; brakes remove 223 kJ.",
    );
  });

  it("omits brakes when none are used and is empty without a preview", () => {
    expect(describeFinishOption({ ...finishOptions, previewedIndex: 0 }, "metric")).toBe(
      "Option 1 of 2: 2 pieces (OffsetLeft, OneTileStraight). Arrives at 31 km/h.",
    );
    expect(describeFinishOption({ ...finishOptions, previewedIndex: null }, "metric")).toBe("");
  });
});
