/** A tool consumes normalized pointer actions and reports whether it handled them. */
export interface Tool<Action, State> {
  state(): State;
  handle(action: Action): boolean;
  cancel(): void;
}

export type DragPhase = "start" | "move" | "end" | "cancel";

/** Device-independent events after the caller has picked the relevant world location. */
export interface InteractiveTool<Activation, Pointer, Key, State> {
  state(): State;
  activate(selection: Activation): void;
  deactivate(): void;
  pointerMove(pointer: Pointer): boolean;
  click(pointer: Pointer): boolean;
  drag(phase: DragPhase, pointer: Pointer): boolean;
  key(key: Key): boolean;
}
