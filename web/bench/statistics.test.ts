import { describe, expect, it } from "vitest";
import { summarizeTimings } from "./statistics";

describe("timing statistics", () => {
  it("reports the arithmetic mean and nearest-rank 95th percentile", () => {
    expect(summarizeTimings([8, 1, 4, 3, 2, 5, 7, 6, 9, 10])).toEqual({
      meanMs: 5.5,
      p95Ms: 10,
    });
  });

  it("rejects an empty sample", () => {
    expect(() => summarizeTimings([])).toThrow(RangeError);
  });
});
