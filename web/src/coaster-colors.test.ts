import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { COASTER_COLOR_NAMES, COASTER_COLORS, isCoasterColorName, parseCoasterColors, repaint } from "./coaster-colors";
import { gradeColor, hexColor, lightingAt } from "./render/daylight";

describe("coaster colour palette", () => {
  it("offers a dozen distinct inks under the same names, in the same order, as the simulation", () => {
    const simulationSource = readFileSync(new URL("../../crates/sim/src/track/colors.rs", import.meta.url), "utf8");
    const simulationNames = [...simulationSource.matchAll(/Self::\w+ => "(\w+)"/g)].map((match) => match[1]);
    expect(COASTER_COLOR_NAMES).toEqual(simulationNames);
    expect(COASTER_COLOR_NAMES).toHaveLength(12);
    expect(new Set(Object.values(COASTER_COLORS).map(({ hex }) => hex)).size).toBe(12);
  });

  it("keeps every colour distinguishable from the dark ground at night once floodlit", () => {
    const night = lightingAt(0);
    const ground = gradeColor(hexColor("#a6dc6c"), night);
    for (const { hex } of Object.values(COASTER_COLORS)) {
      const lit = gradeColor(hexColor(hex), night, 0.4);
      expect(Math.max(...lit.map((channel, index) => Math.abs(channel - ground[index])))).toBeGreaterThan(0.08);
    }
  });
});

describe("coaster colour schemes", () => {
  it("parse the worker's names and refuse anything outside the palette", () => {
    expect(parseCoasterColors("teal\tcream\tcoral")).toEqual({ track: "teal", supports: "cream", trains: "coral" });
    expect(() => parseCoasterColors("teal\tcream")).toThrow();
    expect(() => parseCoasterColors("teal\t#ffffff\tcoral")).toThrow();
    expect(() => parseCoasterColors("teal\tconstructor\tcoral")).toThrow();
    expect(isCoasterColorName("toString")).toBe(false);
  });

  it("repaint one part and leave the others", () => {
    const scheme = parseCoasterColors("coral\ttimber\tsage");
    expect(repaint(scheme, "supports", "charcoal")).toEqual({ track: "coral", supports: "charcoal", trains: "sage" });
    expect(scheme.supports).toBe("timber");
    expect(() => repaint(scheme, "track", "gold")).toThrow();
  });
});
