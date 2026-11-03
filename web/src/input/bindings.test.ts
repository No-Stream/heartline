import { describe, expect, it, vi } from "vitest";
import { bindInput } from "./bindings";
import type { InputAction } from "./actions";

class Surface extends EventTarget {
  width = 800;
  height = 600;
  getBoundingClientRect(): DOMRect { return { left: 10, top: 20, width: 400, height: 300 } as DOMRect; }
  setPointerCapture = vi.fn();
  releasePointerCapture = vi.fn();
}
function event(type: string, properties: Record<string, number | string | boolean>): Event {
  return Object.assign(new Event(type, { cancelable: true }), properties);
}

describe("input bindings", () => {
  it("turns pointer gestures into pan and select actions in canvas coordinates", () => {
    const surface = new Surface();
    const actions: InputAction[] = [];
    const unbind = bindInput(surface as unknown as HTMLCanvasElement, (action) => actions.push(action));
    surface.dispatchEvent(event("pointerdown", { pointerId: 1, button: 0, clientX: 60, clientY: 70 }));
    surface.dispatchEvent(event("pointerup", { pointerId: 1, clientX: 60, clientY: 70 }));
    surface.dispatchEvent(event("pointerdown", { pointerId: 2, button: 0, clientX: 60, clientY: 70 }));
    surface.dispatchEvent(event("pointermove", { pointerId: 2, clientX: 70, clientY: 75 }));
    surface.dispatchEvent(event("pointerup", { pointerId: 2, clientX: 70, clientY: 75 }));
    expect(actions).toEqual([
      { type: "select", position: { x: 100, y: 100 } },
      { type: "pan", dx: 20, dy: 10 },
    ]);
    unbind();
    surface.dispatchEvent(event("wheel", { clientX: 60, clientY: 70, deltaY: 1, deltaMode: 0 }));
    expect(actions).toHaveLength(2);
  });

  it("maps wheel and keyboard to abstract zoom and rotation actions", () => {
    const surface = new Surface();
    const actions: InputAction[] = [];
    bindInput(surface as unknown as HTMLCanvasElement, (action) => actions.push(action));
    surface.dispatchEvent(event("wheel", { clientX: 60, clientY: 70, deltaY: -100, deltaMode: 0 }));
    surface.dispatchEvent(event("keydown", { key: "q", repeat: false }));
    surface.dispatchEvent(event("keydown", { key: "e", repeat: false }));
    expect(actions[0]).toEqual({ type: "zoom", factor: Math.exp(0.1), anchor: { x: 100, y: 100 } });
    expect(actions.slice(1)).toEqual([{ type: "rotate", turns: -1 }, { type: "rotate", turns: 1 }]);
  });
  it("maps construction shortcuts through the action layer", () => {
    const surface = new Surface();
    const actions: InputAction[] = [];
    bindInput(surface as unknown as HTMLCanvasElement, (action) => actions.push(action));
    for (const key of ["b", "Backspace", "c", "t"]) surface.dispatchEvent(event("keydown", { key, repeat: false }));
    expect(actions).toEqual([
      { type: "construction-append" }, { type: "construction-undo" },
      { type: "construction-close" }, { type: "construction-test" },
    ]);
  });
});
