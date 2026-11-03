import { describe, expect, it } from "vitest";
import { advanceDayPhase, DAY_LENGTH_TICKS, formatClock, gradeColor, hexColor, lightingAt, sunAt } from "./daylight";

const luminance = ([red, green, blue]: readonly number[]): number => 0.299 * red + 0.587 * green + 0.114 * blue;
const paper = hexColor("#f2eadb");
const grass = hexColor("#8a9a6c");

describe("day clock", () => {
  it("takes between 20 and 30 real minutes at the 30 Hz simulation rate", () => {
    const minutes = DAY_LENGTH_TICKS / 30 / 60;
    expect(minutes).toBeGreaterThanOrEqual(20);
    expect(minutes).toBeLessThanOrEqual(30);
  });

  it("advances with simulation ticks, wraps at midnight and scales with speed", () => {
    expect(advanceDayPhase(0.5, DAY_LENGTH_TICKS / 4, 1)).toBeCloseTo(0.75);
    expect(advanceDayPhase(0.9, DAY_LENGTH_TICKS / 5, 1)).toBeCloseTo(0.1);
    expect(advanceDayPhase(0.5, DAY_LENGTH_TICKS / 40, 10)).toBeCloseTo(0.75);
    expect(advanceDayPhase(0.3, 0, 60)).toBe(0.3);
  });

  it("rejects ticks running backwards and bad phases", () => {
    expect(() => advanceDayPhase(0.5, -1, 1)).toThrow();
    expect(() => advanceDayPhase(1.2, 1, 1)).toThrow();
    expect(() => lightingAt(Number.NaN)).toThrow();
  });

  it("formats the phase as a 24-hour clock", () => {
    expect(formatClock(0)).toBe("00:00");
    expect(formatClock(0.5)).toBe("12:00");
    expect(formatClock(0.75)).toBe("18:00");
    expect(formatClock(0.999)).toBe("23:58");
  });
});

describe("sun", () => {
  it("rises at six, is highest at noon and is below the horizon at night", () => {
    expect(sunAt(0.25).elevation).toBeCloseTo(0, 5);
    expect(sunAt(0.5).elevation).toBeGreaterThan(sunAt(0.35).elevation);
    expect(sunAt(0.5).elevation).toBeLessThan(Math.PI / 2);
    expect(sunAt(0).elevation).toBeLessThan(0);
  });

  it("moves across the sky, so shadows point the other way in the evening", () => {
    const morning = sunAt(0.33).shadowDirection;
    const evening = sunAt(0.67).shadowDirection;
    expect(morning.x * evening.x + morning.y * evening.y).toBeLessThan(0);
    expect(Math.hypot(morning.x, morning.y)).toBeCloseTo(1);
  });
});

describe("colour grade", () => {
  it("leaves noon untouched", () => {
    const noon = lightingAt(0.5);
    for (const color of [paper, grass, hexColor("#dc5c3e"), hexColor("#3a2e58")]) {
      gradeColor(color, noon).forEach((channel, index) => expect(channel).toBeCloseTo(color[index], 5));
    }
    expect(noon.lightsOn).toBe(0);
  });

  it("warms the land at dusk without darkening it much", () => {
    const [red, , blue] = gradeColor(grass, lightingAt(0.75));
    expect(red - blue).toBeGreaterThan(grass[0] - grass[2]);
    expect(luminance(gradeColor(paper, lightingAt(0.75)))).toBeGreaterThan(0.5);
  });

  it("turns night into a deep blue-plum rather than black, and keeps paper above grass", () => {
    const night = lightingAt(0);
    const gradedGrass = gradeColor(grass, night);
    const gradedPaper = gradeColor(paper, night);
    expect(gradedGrass[2]).toBeGreaterThan(gradedGrass[1]);
    expect(gradedGrass[2]).toBeGreaterThan(0.2);
    expect(luminance(gradedGrass)).toBeLessThan(luminance(grass) * 0.7);
    expect(luminance(gradedPaper) - luminance(gradedGrass)).toBeGreaterThan(0.15);
    expect(night.lightsOn).toBe(1);
  });

  it("lets lit surfaces keep more of their colour at night", () => {
    const night = lightingAt(0);
    expect(luminance(gradeColor(paper, night, 0.4))).toBeGreaterThan(luminance(gradeColor(paper, night)));
    gradeColor(paper, night, 1).forEach((channel, index) => expect(channel).toBeCloseTo(paper[index], 5));
  });

  it("changes smoothly through the day", () => {
    for (let step = 0; step < 400; step++) {
      const before = gradeColor(grass, lightingAt(step / 400));
      const after = gradeColor(grass, lightingAt((step + 1) / 400 % 1));
      expect(Math.max(...before.map((channel, index) => Math.abs(channel - after[index])))).toBeLessThan(0.04);
    }
  });
});
