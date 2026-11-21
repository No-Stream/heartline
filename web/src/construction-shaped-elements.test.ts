import { describe, expect, it } from "vitest";
import { shapedElementReadout } from "./candidate-force-readout";
import { defaultShapeSelectionId, shapeSelectionId, type ShapedElementSelection } from "./construction-shaped-elements";

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

  it("shows a typed shaped-element refusal without inventing a force readout", () => {
    expect(shapedElementReadout({
      selection: "shape:hill:ejector:-1.2:auto",
      kind: "hill",
      feel: "ejector",
      hand: null,
      strengthG: -1.2,
      fixedSize: null,
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
      spec: null,
      refusal: { kind: "geometryDigestMismatch", expected: 1n, actual: 2n },
      pieceShape: null,
    }, "metric")).toEqual([
      "Saved shaped geometry does not match its recipe.",
    ]);
  });
});
