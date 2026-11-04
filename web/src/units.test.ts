import { describe, expect, it } from "vitest";
import {
  displayToMetres,
  displaySpeedToMetresPerSecond,
  formatLength,
  formatSpeed,
  metresPerSecondToDisplay,
  metresToDisplay,
  metresToInput,
  speedUnitLabel,
  unitLabel,
} from "./units";

describe("unit conversions", () => {
  it("converts lengths between metres and feet without rounding the stored value", () => {
    expect(metresToDisplay(10, "imperial")).toBeCloseTo(32.80839895);
    expect(displayToMetres(32.80839895, "imperial")).toBeCloseTo(10);
    expect(metresToDisplay(10, "metric")).toBe(10);
    expect(displayToMetres(10, "metric")).toBe(10);
  });

  it("formats lengths with the selected unit and requested precision", () => {
    expect(formatLength(10, "imperial")).toBe("33 ft");
    expect(formatLength(10, "imperial", 1)).toBe("32.8 ft");
    expect(formatLength(1234.567, "metric", 2)).toBe("1,234.57 m");
    expect(unitLabel("imperial")).toBe("ft");
    expect(unitLabel("metric")).toBe("m");
  });

  it("rounds imperial input displays to whole feet and returns valid station heights", () => {
    for (let stationHeight = 4; stationHeight <= 100; stationHeight += 2) {
      const displayedFeet = metresToInput(stationHeight, "imperial");
      const roundTrippedHeight = Math.round(displayToMetres(displayedFeet, "imperial") / 2) * 2;
      expect(Number.isInteger(displayedFeet)).toBe(true);
      expect(roundTrippedHeight).toBe(stationHeight);
    }

    expect(metresToInput(12, "imperial")).toBe(39);
    expect(metresToInput(12, "metric")).toBe(12);
  });

  it("converts and formats speeds in miles or kilometres per hour", () => {
    expect(metresPerSecondToDisplay(12, "imperial")).toBeCloseTo(26.8433);
    expect(metresPerSecondToDisplay(12, "metric")).toBeCloseTo(43.2);
    expect(displaySpeedToMetresPerSecond(26.8432355, "imperial")).toBeCloseTo(12);
    expect(displaySpeedToMetresPerSecond(43.2, "metric")).toBeCloseTo(12);
    expect(formatSpeed(12, "imperial")).toBe("26.8 mph");
    expect(formatSpeed(12, "imperial", 3)).toBe("26.8 mph");
    expect(formatSpeed(12, "metric")).toBe("43.2 km/h");
    expect(speedUnitLabel("imperial")).toBe("mph");
    expect(speedUnitLabel("metric")).toBe("km/h");
  });

  it("converts nonzero launch speed bounds for both unit systems", () => {
    expect(metresPerSecondToDisplay(3, "metric")).toBeCloseTo(10.8);
    expect(metresPerSecondToDisplay(3, "imperial")).toBeCloseTo(6.7108);
    expect(metresPerSecondToDisplay(240 / 3.6, "metric")).toBeCloseTo(240);
    expect(metresPerSecondToDisplay(240 / 3.6, "imperial")).toBeCloseTo(149.129);
  });
});
