import { describe, expect, it } from "vitest";
import { shapedElementReadout } from "./candidate-force-readout";
import { shapedPaletteChoices } from "./construction-choices";
import {
  defaultInversionSelectionId,
  defaultShapeSelectionId,
  shapeSelectionId,
  steppedShapedSelection,
  type ShapedElementSelection,
} from "./construction-shaped-elements";

describe("shaped element selections", () => {
  it("serializes hill feel, adjustable strength, and automatic size", () => {
    const selection: ShapedElementSelection = {
      kind: "hill",
      feel: "floater",
      strengthG: -0.5,
      size: { kind: "auto" },
    };

    expect(shapeSelectionId(selection)).toBe("shape:hill:floater:-0.5:auto");
  });

  it("serializes an explicit hill height while leaving the span automatic", () => {
    const selection: ShapedElementSelection = {
      kind: "hill",
      feel: "ejector",
      strengthG: -1.0,
      size: { kind: "height", heightMetres: 36 },
    };

    expect(shapeSelectionId(selection)).toBe("shape:hill:ejector:-1:fixed-36");
  });

  it("serializes fully banked turns with automatic and fixed radii", () => {
    const automatic: ShapedElementSelection = {
      kind: "turn",
      hand: "left",
      feel: "fullyBanked",
      size: { kind: "auto" },
    };
    const fixed: ShapedElementSelection = {
      ...automatic,
      size: { kind: "radius", radiusMetres: 64 },
    };

    expect(shapeSelectionId(automatic)).toBe("shape:turn:left:fullyBanked:auto");
    expect(shapeSelectionId(fixed)).toBe("shape:turn:left:fullyBanked:fixed-64");
  });

  it("keeps default palette selection separate from explicit hill intent", () => {
    const selection: ShapedElementSelection = {
      kind: "hill",
      feel: "strongFloater",
      strengthG: -0.85,
      size: { kind: "auto" },
    };

    expect(defaultShapeSelectionId("strongFloater")).toBe("shape:hill:strongFloater:default:auto");
    expect(shapeSelectionId(selection)).toBe("shape:hill:strongFloater:-0.85:auto");
  });

  it("shows the train-calibrated hill forces, design speed, and resolved size", () => {
    expect(shapedElementReadout({
      selection: "shape:hill:floater:-0.5:auto",
      kind: "hill",
      feel: "floater",
      hand: null,
      strengthG: -0.5,
      fixedSize: null,
      modelName: "Horizon Hawk",
      exceedsModelFeel: false,
      spec: {
        kind: "hill",
        designSpeedMps: 34,
        cars: 8,
        targetG: -0.5,
        deliveredG: -0.48,
        verticalG: null,
        lateralG: null,
        heightM: 36,
        spanM: 96,
        radiusM: null,
        bankDegrees: null,
        topFeel: null,
        topG: null,
        peakRollRateDegS: null,
        steelStructure: false,
        geometryDigest: "0123456789abcdef",
      },
      refusal: null,
      pieceShape: null,
    }, "metric")).toEqual([
      "Design speed: 122 km/h · 8-car train",
      "Airtime target: −0.5 g · calibrated: −0.48 g",
      "Resolved size: 36 m high × 96 m span",
    ]);
  });

  it("adds a mild feel note when a shaped element pushes past the model's typical feel inside the ASTM envelope", () => {
    expect(shapedElementReadout({
      selection: "shape:hill:ejector:-1.2:auto",
      kind: "hill",
      feel: "ejector",
      hand: null,
      strengthG: -1.2,
      fixedSize: null,
      modelName: "Horizon Hawk",
      exceedsModelFeel: true,
      spec: {
        kind: "hill",
        designSpeedMps: 34,
        cars: 8,
        targetG: -1.2,
        deliveredG: -1.19,
        verticalG: null,
        lateralG: null,
        heightM: 30,
        spanM: 120,
        radiusM: null,
        bankDegrees: null,
        topFeel: null,
        topG: null,
        peakRollRateDegS: null,
        steelStructure: false,
        geometryDigest: "0123456789abcdef",
      },
      refusal: null,
      pieceShape: null,
    }, "metric")).toEqual([
      "Design speed: 122 km/h · 8-car train",
      "Airtime target: −1.2 g · calibrated: −1.19 g",
      "Resolved size: 30 m high × 120 m span",
      "Within the comfort envelope, stronger than a typical Horizon Hawk.",
    ]);
  });

  it("shows a typed shaped-element refusal without inventing a force readout", () => {
    expect(shapedElementReadout({
      selection: "shape:hill:ejector:-1.2:auto",
      kind: "hill",
      feel: "ejector",
      hand: null,
      strengthG: -1.2,
      fixedSize: null,
      modelName: "Horizon Hawk",
      exceedsModelFeel: false,
      spec: null,
      refusal: { kind: "stall", entrySpeedMps: 14, heightM: 28 },
      pieceShape: null,
    }, "metric")).toEqual([
      "Would stall at 50 km/h; tallest feasible hill is 28 m.",
    ]);
  });

  it("explains a saved shaped geometry digest mismatch", () => {
    expect(shapedElementReadout({
      selection: "shape:turn:left:fullyBanked:fixed-68",
      kind: "turn",
      feel: "fullyBanked",
      hand: "left",
      strengthG: null,
      fixedSize: 68,
      modelName: "Horizon Hawk",
      exceedsModelFeel: false,
      spec: null,
      refusal: { kind: "geometryDigestMismatch", expected: 1n, actual: 2n },
      pieceShape: null,
    }, "metric")).toEqual([
      "Saved shaped geometry does not match its recipe.",
    ]);
  });

  it("serializes and steps shaped vertical loops, zero-g rolls, and Raven turns", () => {
    expect(defaultInversionSelectionId("loop", "left")).toBe("shape:loop:left:default:neutral");
    expect(defaultInversionSelectionId("zeroGRoll", "right")).toBe("shape:zeroGRoll:right:default");
    expect(defaultInversionSelectionId("ravenTurn", "left")).toBe("shape:ravenTurn:left:default:neutral");

    expect(shapeSelectionId({ kind: "loop", hand: "left", peakG: 4.0, topFeel: "floaty" })).toBe("shape:loop:left:4:floaty");
    expect(shapeSelectionId({ kind: "zeroGRoll", hand: "right", peakG: 3.5 })).toBe("shape:zeroGRoll:right:3.5");
    expect(shapeSelectionId({ kind: "ravenTurn", hand: "left", peakG: 4.2, topFeel: "pressedIn" })).toBe("shape:ravenTurn:left:4.2:pressedIn");

    const loopWire = {
      selection: "shape:loop:left:4:neutral",
      kind: "loop" as const,
      feel: "neutral",
      hand: "left" as const,
      strengthG: 4.0,
      fixedSize: null,
      modelName: "Velocity Viper",
      exceedsModelFeel: false,
      spec: {
        kind: "loop" as const,
        designSpeedMps: 28,
        cars: 4,
        targetG: 4.0,
        deliveredG: 4.01,
        verticalG: 4.01,
        lateralG: null,
        heightM: 28,
        spanM: 60,
        radiusM: null,
        bankDegrees: null,
        topFeel: "neutral" as const,
        topG: 0.96,
        peakRollRateDegS: null,
        steelStructure: false,
        geometryDigest: "0123456789abcdef",
      },
      refusal: null,
      pieceShape: null,
    };
    expect(steppedShapedSelection(loopWire, "peakG", 1)).toEqual({ kind: "loop", hand: "left", peakG: 4.2, topFeel: "neutral" });
    expect(steppedShapedSelection(loopWire, "peakG", -1)).toEqual({ kind: "loop", hand: "left", peakG: 3.8, topFeel: "neutral" });
    expect(steppedShapedSelection(loopWire, "topFeel", -1)).toEqual({ kind: "loop", hand: "left", peakG: 4.0, topFeel: "floaty" });
    expect(steppedShapedSelection(loopWire, "topFeel", 1)).toEqual({ kind: "loop", hand: "left", peakG: 4.0, topFeel: "pressedIn" });
    expect(shapedElementReadout(loopWire, "metric")).toEqual([
      "Design speed: 101 km/h · 4-car train",
      "Peak target: 4 g · calibrated: 4.01 g · top: 0.96 g (Neutral)",
      "Resolved size: 28 m high × 60 m span",
    ]);

    expect(shapedElementReadout({
      ...loopWire,
      modelName: "Briar Blitz",
      exceedsModelFeel: true,
      spec: { ...loopWire.spec, steelStructure: true, targetG: 4.2, deliveredG: 4.18 },
    }, "metric")).toEqual([
      "Design speed: 101 km/h · 4-car train",
      "Peak target: 4.2 g · calibrated: 4.18 g · top: 0.96 g (Neutral)",
      "Resolved size: 28 m high × 60 m span · steel structure",
      "Within the comfort envelope, stronger than a typical Briar Blitz.",
    ]);
  });

  it("filters shaped inversion palette choices by model capabilities", () => {
    expect(shapedPaletteChoices([]).filter((c) => c.group === "inversion")).toEqual([]);
    expect(shapedPaletteChoices(["VerticalLoop"]).filter((c) => c.group === "inversion").map((c) => c.id)).toEqual([
      "shape:loop:left:default:neutral",
      "shape:loop:right:default:neutral",
    ]);
    expect(shapedPaletteChoices(["VerticalLoop", "Corkscrew", "HeartlineRoll"]).filter((c) => c.group === "inversion").map((c) => c.id)).toEqual([
      "shape:loop:left:default:neutral",
      "shape:loop:right:default:neutral",
      "shape:zeroGRoll:left:default",
      "shape:zeroGRoll:right:default",
      "shape:ravenTurn:left:default:neutral",
      "shape:ravenTurn:right:default:neutral",
    ]);
  });
});
