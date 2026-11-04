import { describe, expect, it } from "vitest";
import { selectGameBarFitLevel } from "./game-bar-fit";

describe("selectGameBarFitLevel", () => {
  it("keeps the least important available content when it fits", () => {
    const widths = [1400, 1320, 1260, 1180, 1100, 980, 900, 820, 760];
    expect(selectGameBarFitLevel(1000, (level) => widths[level]!)).toBe(5);
  });

  it("uses the final collapse level when even the preferred controls do not fit", () => {
    const widths = [1400, 1320, 1260, 1180, 1100, 980, 900, 820, 760];
    expect(selectGameBarFitLevel(700, (level) => widths[level]!)).toBe(8);
  });

  it("does not collapse content when the full bar fits", () => {
    expect(selectGameBarFitLevel(1500, (level) => 1400 - level * 80)).toBe(0);
  });
});
