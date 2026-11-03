import type { InputAction, ScreenPoint } from "./actions";

/** Bind a canvas to actions. Pointer coordinates are scaled to its backing-store pixels. */
export function bindInput(canvas: HTMLCanvasElement, dispatch: (action: InputAction) => void): () => void {
  let pointer: { id: number; start: ScreenPoint; last: ScreenPoint; dragging: boolean } | undefined;
  const point = (event: MouseEvent): ScreenPoint => {
    const bounds = canvas.getBoundingClientRect();
    return { x: (event.clientX - bounds.left) * canvas.width / bounds.width, y: (event.clientY - bounds.top) * canvas.height / bounds.height };
  };
  const down = (event: PointerEvent): void => {
    if (pointer || event.button !== 0) return;
    const position = point(event);
    pointer = { id: event.pointerId, start: position, last: position, dragging: false };
    canvas.setPointerCapture(event.pointerId);
  };
  const move = (event: PointerEvent): void => {
    if (!pointer) { dispatch({ type: "hover", position: point(event) }); return; }
    if (pointer.id !== event.pointerId) return;
    const position = point(event);
    if (!pointer.dragging && Math.hypot(position.x - pointer.start.x, position.y - pointer.start.y) < 4) return;
    pointer.dragging = true;
    dispatch({ type: "pan", dx: position.x - pointer.last.x, dy: position.y - pointer.last.y });
    pointer.last = position;
  };
  const up = (event: PointerEvent): void => {
    if (!pointer || pointer.id !== event.pointerId) return;
    if (!pointer.dragging) dispatch({ type: "select", position: point(event) });
    canvas.releasePointerCapture(event.pointerId);
    pointer = undefined;
  };
  const cancel = (event: PointerEvent): void => {
    if (pointer?.id === event.pointerId) pointer = undefined;
  };
  const wheel = (event: WheelEvent): void => {
    event.preventDefault();
    const pixels = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? canvas.height : 1);
    dispatch({ type: "zoom", factor: Math.exp(-pixels * 0.001), anchor: point(event) });
  };
  const key = (event: KeyboardEvent): void => {
    let action: InputAction | undefined;
    switch (event.key.toLowerCase()) {
      case "b": if (!event.repeat) action = { type: "construction-append" }; break;
      case "backspace": if (!event.repeat) action = { type: "construction-undo" }; break;
      case "c": if (!event.repeat) action = { type: "construction-close" }; break;
      case "t": if (!event.repeat) action = { type: "construction-test" }; break;
      case "q": if (!event.repeat) action = { type: "rotate", turns: -1 }; break;
      case "e": if (!event.repeat) action = { type: "rotate", turns: 1 }; break;
      case "arrowleft": action = { type: "pan", dx: 32, dy: 0 }; break;
      case "arrowright": action = { type: "pan", dx: -32, dy: 0 }; break;
      case "arrowup": action = { type: "pan", dx: 0, dy: 32 }; break;
      case "arrowdown": action = { type: "pan", dx: 0, dy: -32 }; break;
      case "+": case "=": action = { type: "zoom", factor: 1.1, anchor: { x: canvas.width / 2, y: canvas.height / 2 } }; break;
      case "-": action = { type: "zoom", factor: 1 / 1.1, anchor: { x: canvas.width / 2, y: canvas.height / 2 } }; break;
    }
    if (action) { event.preventDefault(); dispatch(action); }
  };
  const leave = (): void => { if (!pointer) dispatch({ type: "hover", position: null }); };
  canvas.addEventListener("pointerleave", leave);
  canvas.addEventListener("pointerdown", down);
  canvas.addEventListener("pointermove", move);
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", cancel);
  canvas.addEventListener("wheel", wheel, { passive: false });
  canvas.addEventListener("keydown", key);
  return () => {
    canvas.removeEventListener("pointerleave", leave);
    canvas.removeEventListener("pointerdown", down);
    canvas.removeEventListener("pointermove", move);
    canvas.removeEventListener("pointerup", up);
    canvas.removeEventListener("pointercancel", cancel);
    canvas.removeEventListener("wheel", wheel);
    canvas.removeEventListener("keydown", key);
  };
}
