import { describe, expect, it } from "vitest";
import { canKeepGuestPanelPosition } from "./placement";

describe("guest panel position", () => {
  const canvas = { left: 0, top: 44, right: 1280, bottom: 720 };
  const panel = { left: 650, top: 100, right: 954, bottom: 650 };

  it("keeps the panel still when a nearby selected guest moves or changes", () => {
    expect(canKeepGuestPanelPosition(canvas, panel, { x: 600, y: 300 })).toBe(true);
    expect(canKeepGuestPanelPosition(canvas, panel, { x: 625, y: 320 })).toBe(true);
  });

  it("moves the panel when the selected guest approaches or enters it", () => {
    expect(canKeepGuestPanelPosition(canvas, panel, { x: 640, y: 320 })).toBe(false);
    expect(canKeepGuestPanelPosition(canvas, panel, { x: 700, y: 320 })).toBe(false);
  });

  it("moves the panel when resizing leaves it outside the canvas", () => {
    expect(canKeepGuestPanelPosition({ ...canvas, right: 900 }, panel, { x: 600, y: 300 })).toBe(false);
  });
});
