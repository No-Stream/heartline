import { describe, expect, it } from "vitest";
import {
  displayToMetres,
  formatLength,
  formatSpeed,
  metresPerSecondToDisplay,
  metresToDisplay,
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
    expect(formatLength(10, "imperial")).toBe("32.8 ft");
    expect(formatLength(1234.567, "metric", 2)).toBe("1,234.57 m");
    expect(unitLabel("imperial")).toBe("ft");
    expect(unitLabel("metric")).toBe("m");
  });

  it("converts and formats speeds in miles or kilometres per hour", () => {
    expect(metresPerSecondToDisplay(12, "imperial")).toBeCloseTo(26.8433);
    expect(metresPerSecondToDisplay(12, "metric")).toBeCloseTo(43.2);
    expect(formatSpeed(12, "imperial")).toBe("26.8 mph");
    expect(formatSpeed(12, "metric")).toBe("43.2 km/h");
    expect(speedUnitLabel("imperial")).toBe("mph");
    expect(speedUnitLabel("metric")).toBe("km/h");
  });
});
