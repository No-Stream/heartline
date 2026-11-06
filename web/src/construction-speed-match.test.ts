import { describe, expect, it } from "vitest";
import { constructionEndSpeed, speedMatchedSelection, speedMatchMessage, speedTurnMessage, speedTurnName, speedTurnSizeLabel } from "./construction-speed-match";

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

describe("speed-matched turn labels and advice", () => {
  const speedTurns = [
    { id: "SpeedTurnLarge16Left", hand: "left", designSpeed: 16 },
    { id: "SpeedTurnLarge20Left", hand: "left", designSpeed: 20 },
    { id: "SpeedTurnLarge24Left", hand: "left", designSpeed: 24 },
  ] as const;
  const turnMatch = (entrySpeed: number | null, recommendedId: string | null, fits: boolean | null) => ({
    entrySpeed, recommendedId, fits, variants: speedTurns.map(({ id }) => ({ id, label: id })),
  });

  it("names each size by its design speed in the player's units", () => {
    expect(speedTurnSizeLabel(16, "metric")).toBe("Banked for 58 km/h");
    expect(speedTurnSizeLabel(24, "metric")).toBe("Banked for 86 km/h");
    expect(speedTurnSizeLabel(16, "imperial")).toBe("Banked for 36 mph");
    expect(speedTurnSizeLabel(24, "imperial")).toBe("Banked for 54 mph");
    expect(speedTurnName("Large left turn banked for 24 m/s", 24, "metric")).toBe("Large left turn banked for 86 km/h");
    expect(speedTurnName("Large right turn banked for 20 m/s", 20, "imperial")).toBe("Large right turn banked for 45 mph");
    expect(() => speedTurnName("Large left turn", 20, "metric")).toThrow();
  });

  it("says which size suits the predicted speed and notes an override", () => {
    expect(speedTurnMessage(turnMatch(19.5, "SpeedTurnLarge20Left", true), "SpeedTurnLarge20Left", speedTurns, "metric"))
      .toBe("Banked for 72 km/h suits the predicted entry speed.");
    expect(speedTurnMessage(turnMatch(19.5, "SpeedTurnLarge20Left", true), "SpeedTurnLarge16Left", speedTurns, "imperial"))
      .toBe("Banked for 45 mph suits the predicted entry speed. You selected the turn banked for 36 mph.");
  });

  it("says plainly when the train is too fast or slow for every size", () => {
    expect(speedTurnMessage(turnMatch(30, "SpeedTurnLarge24Left", false), "SpeedTurnLarge24Left", speedTurns, "metric"))
      .toBe("The train is too fast for every turn banked for speed; the steepest is banked for 86 km/h. Slow the train first with a hill or brakes.");
    expect(speedTurnMessage(turnMatch(12.4, "SpeedTurnLarge16Left", false), "SpeedTurnLarge16Left", speedTurns, "metric"))
      .toBe("The train is a little slow for these banks; banked for 58 km/h is the closest.");
    expect(speedTurnMessage(turnMatch(20, null, false), "SpeedTurnLarge16Left", speedTurns, "metric"))
      .toBe("No turn banked for speed fits here.");
    expect(speedTurnMessage(turnMatch(null, null, null), "SpeedTurnLarge16Left", speedTurns, "metric"))
      .toBe("The train does not reach this piece, so a size cannot be recommended.");
  });
});
