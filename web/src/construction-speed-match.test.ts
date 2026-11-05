import { describe, expect, it } from "vitest";
import { constructionEndSpeed, speedMatchedSelection, speedMatchMessage } from "./construction-speed-match";

const match = {
  entrySpeed: 20,
  recommendedId: "LargeLoop",
  fits: true,
  variants: [{ id: "Loop", label: "Loop" }, { id: "LargeLoop", label: "Large loop" }],
};

describe("speed-matched construction selection", () => {
  it("selects the recommendation on a family choice and preserves explicit size choices", () => {
    expect(speedMatchedSelection("Loop", match, false)).toBe("LargeLoop");
    expect(speedMatchedSelection("Loop", match, true)).toBe("Loop");
    expect(speedMatchedSelection("Loop", null, false)).toBe("Loop");
    expect(speedMatchedSelection("Loop", { ...match, recommendedId: null }, false)).toBe("Loop");
  });

  it("explains when no available size fits or no estimate is available", () => {
    expect(speedMatchMessage({ ...match, fits: false }, "Loop")).toBe("No available size fits the estimated entry speed. Large loop has the nearest rated range.");
    expect(speedMatchMessage({ ...match, entrySpeed: null, recommendedId: null, fits: null }, "Loop")).toBe("The train does not reach this piece, so a size cannot be recommended.");
    expect(speedMatchMessage(match, "LargeLoop")).toBe("Large loop is recommended: its rated range contains the entry speed with the greatest margin to either edge.");
    expect(speedMatchMessage(match, "Loop")).toBe("Large loop is recommended: its rated range contains the entry speed with the greatest margin to either edge. You selected Loop.");
  });
});

it("uses the latest end speed only when the placed-track estimate reaches the end", () => {
  const sample = { distance: 10, x: 0, y: 0, z: 0, speed: 24, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false };
  const preview = { status: "reachedEnd" as const, samples: [sample], completed: true, lastPlaced: null, launches: [], limits: { minimumVerticalG: -1, maximumVerticalG: 4, maximumLateralG: 1, maximumLongitudinalG: 1 } };
  expect(constructionEndSpeed(preview)).toBe(24);
  expect(constructionEndSpeed({ ...preview, status: "stalled" })).toBeUndefined();
  expect(constructionEndSpeed({ ...preview, status: "timedOut" })).toBeUndefined();
  expect(constructionEndSpeed({ ...preview, samples: [] })).toBeUndefined();
  expect(constructionEndSpeed(null)).toBeUndefined();
});
