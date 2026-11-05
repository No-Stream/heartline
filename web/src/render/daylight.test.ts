import { describe, expect, it } from "vitest";
import { advanceDayPhase, BACKDROPS, DAY_LENGTH_TICKS, formatClock, gradeColor, hexColor, lightingAt, lightInPool, printOnBackdrop, radialPoolFalloff, sunAt, type Rgb } from "./daylight";

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

  it("turns night into a dark slate blue rather than black, and keeps paper above grass", () => {
    const night = lightingAt(0);
    const gradedGrass = gradeColor(grass, night);
    const gradedPaper = gradeColor(paper, night);
    expect(gradedGrass[2]).toBeGreaterThan(gradedGrass[0]);
    expect(gradedGrass[2]).toBeGreaterThan(0.07);
    expect(luminance(gradedGrass)).toBeLessThan(0.18);
    expect(luminance(gradedPaper) - luminance(gradedGrass)).toBeGreaterThan(0.08);
    expect(night.lightsOn).toBe(1);
  });

  it("darkens the park steadily from noon through dusk into night", () => {
    const grassAt = (phase: number): number => luminance(gradeColor(grass, lightingAt(phase)));
    expect(grassAt(0.5)).toBeGreaterThan(grassAt(0.7));
    expect(grassAt(0.7)).toBeGreaterThan(grassAt(0.76));
    expect(grassAt(0.76)).toBeGreaterThan(grassAt(0.81));
    expect(grassAt(0.81)).toBeGreaterThan(grassAt(0.9));
    expect(grassAt(0.76)).toBeGreaterThan(0.3);
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

describe("lamp pools", () => {
  const night = lightingAt(0);
  const lamp: Rgb = hexColor("#ffe2ad");
  const pool = (strength: number): Rgb => [lamp[0] * strength, lamp[1] * strength, lamp[2] * strength];
  const path = hexColor("#fff0c7");

  it("uses a smooth radial falloff from the lamp foot to zero at the radius", () => {
    const samples = [0, 0.2, 0.4, 0.6, 0.8, 0.9, 1].map(radialPoolFalloff);
    expect(samples[0]).toBe(1);
    expect(samples.every((strength, index) => index === 0 || strength < samples[index - 1])).toBe(true);
    expect(radialPoolFalloff(0.2)).toBeGreaterThan(radialPoolFalloff(0.8) * 3);
    expect(samples.at(-1)).toBe(0);
  });

  it("leave unlit surfaces at the night grade", () => {
    expect(lightInPool(gradeColor(path, night), path, [0, 0, 0])).toEqual(gradeColor(path, night));
    expect(lightInPool(gradeColor(path, night), path, pool(0.05))).toEqual(gradeColor(path, night));
  });

  it("fade lamp light continuously from the pool edge toward its brighter centre", () => {
    const graded = gradeColor(path, night);
    const edge = lightInPool(graded, path, pool(0.2));
    const rim = lightInPool(graded, path, pool(0.3));
    const core = lightInPool(graded, path, pool(0.9));
    expect(rim).not.toEqual(edge);
    expect(core).not.toEqual(rim);
    rim.forEach((channel, index) => expect(channel).toBeGreaterThanOrEqual(edge[index]));
    core.forEach((channel, index) => expect(channel).toBeGreaterThanOrEqual(rim[index]));
    expect(luminance(rim)).toBeGreaterThan(luminance(graded) * 1.4);
    expect(luminance(core)).toBeGreaterThan(luminance(rim) * 1.5);
    expect(luminance(core)).toBeGreaterThan(5 * luminance(gradeColor(grass, night)));
    expect(core[0]).toBeGreaterThan(core[2]);
  });

  it("never darken a surface below its grade, as at dusk when the land is still bright", () => {
    const dusk = lightingAt(0.76);
    const graded = gradeColor(paper, dusk);
    lightInPool(graded, paper, pool(0.3)).forEach((channel, index) => expect(channel).toBeGreaterThanOrEqual(graded[index]));
  });
});

describe("backdrop", () => {
  it("leaves the light sky and paper exactly as the day grades them", () => {
    for (const phase of [0, 0.3, 0.5, 0.76]) {
      expect(printOnBackdrop(lightingAt(phase), BACKDROPS.light)).toEqual({ lighting: lightingAt(phase), paper: gradeColor(paper, lightingAt(phase)) });
    }
  });

  it("prints a dark sky and page on dark stock at every hour, keeps the sky lighter by day, and leaves the park's grade alone", () => {
    for (let step = 0; step < 48; step++) {
      const lighting = lightingAt(step / 48);
      const dark = printOnBackdrop(lighting, BACKDROPS.dark);
      expect(Math.max(luminance(dark.lighting.skyTop), luminance(dark.lighting.skyHorizon))).toBeLessThan(0.2);
      expect(luminance(dark.paper)).toBeLessThan(0.06);
      expect({ ...dark.lighting, skyTop: lighting.skyTop, skyHorizon: lighting.skyHorizon }).toEqual(lighting);
    }
    const skyTop = (phase: number): number => luminance(printOnBackdrop(lightingAt(phase), BACKDROPS.dark).lighting.skyTop);
    expect(skyTop(0.5)).toBeGreaterThan(skyTop(0));
  });
});
