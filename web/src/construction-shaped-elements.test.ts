import { describe, expect, it } from "vitest";
import { shapedElementReadout } from "./candidate-force-readout";
import { shapeSelectionId, type ShapedElementSelection } from "./construction-shaped-elements";

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

  it("preserves model defaults when a hill strength is not overridden", () => {
    const selection: ShapedElementSelection = {
      kind: "hill",
      feel: "strongFloater",
      strengthG: null,
      size: { kind: "auto" },
    };

    expect(shapeSelectionId(selection)).toBe("shape:hill:strongFloater:default:auto");
  });

  it("shows the train-calibrated hill forces, design speed, and resolved size", () => {
    expect(shapedElementReadout({
      selection: "shape:hill:floater:default:auto",
      kind: "hill",
      feel: "floater",
      hand: null,
      strengthG: null,
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
});
