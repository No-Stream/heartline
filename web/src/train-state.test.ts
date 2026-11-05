import { describe, expect, it } from "vitest";
import { parseTrainStates } from "./train-states";

describe("train state records", () => {
  it("accepts empty records and valid six-float records", () => {
    expect(parseTrainStates(new Float32Array())).toEqual([]);
    expect(() => parseTrainStates(new Float32Array([
      0, 12.5, 1, 1.2, -0.3, 1,
      1, 4, 0, 0.8, 0.1, 0,
    ]))).not.toThrow();
  });

  it("rejects incomplete or invalid train state records", () => {
    const invalidRecords = [
      new Float32Array([0, 12.5, 1, 1.2, -0.3]),
      new Float32Array([0.5, 12.5, 1, 1.2, -0.3, 1]),
      new Float32Array([0, 12.5, 5, 1.2, -0.3, 1]),
      new Float32Array([0, 12.5, 1, Number.NaN, -0.3, 1]),
      new Float32Array([0, 12.5, 1, 1.2, -0.3, 2]),
    ];

    for (const records of invalidRecords) expect(() => parseTrainStates(records)).toThrow();
  });
});
