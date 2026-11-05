import { describe, expect, it } from "vitest";
import { carsPerTrainControl } from "./cars-per-train";

describe("cars per train control", () => {
  it("shows train capacity and limits changes to the model's range", () => {
    expect(carsPerTrainControl(4, 4, 8, 2, false)).toEqual({
      capacityText: "8 riders per train",
      fewerDisabled: true,
      moreDisabled: false,
    });
    expect(carsPerTrainControl(8, 4, 8, 2, false)).toEqual({
      capacityText: "16 riders per train",
      fewerDisabled: false,
      moreDisabled: true,
    });
  });

  it("disables both controls while changes are unavailable", () => {
    expect(carsPerTrainControl(5, 4, 8, 2, true)).toMatchObject({ fewerDisabled: true, moreDisabled: true });
  });
});
