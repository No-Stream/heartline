import { describe, expect, it } from "vitest";
import { canvasBackingSizeForBounds } from "./canvas-sizing";

describe("canvas backing size", () => {
  it("defers resizing while either CSS bound is zero", () => {
    expect(canvasBackingSizeForBounds({ width: 0, height: 720 }, 2)).toBeNull();
    expect(canvasBackingSizeForBounds({ width: 1280, height: 0 }, 2)).toBeNull();
  });

  it("keeps positive backing dimensions for visible fractional bounds", () => {
    expect(canvasBackingSizeForBounds({ width: 0.25, height: 0.25 }, 1)).toEqual({
      width: 1,
      height: 1,
      pixelRatio: 4,
    });
  });

  it("scales positive CSS bounds by the device pixel ratio", () => {
    expect(canvasBackingSizeForBounds({ width: 640, height: 360 }, 2)).toEqual({
      width: 1280,
      height: 720,
      pixelRatio: 2,
    });
  });
});
