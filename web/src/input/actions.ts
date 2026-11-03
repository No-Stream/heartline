import type { ConstructionAction } from "../construction-protocol";

export interface ScreenPoint {
  readonly x: number;
  readonly y: number;
}

/** Actions describe intent independently of the device that produced them. */
export type InputAction =
  | { readonly type: "pan"; readonly dx: number; readonly dy: number }
  | { readonly type: "zoom"; readonly factor: number; readonly anchor: ScreenPoint }
  | { readonly type: "rotate"; readonly turns: number }
  | { readonly type: "select"; readonly position: ScreenPoint }
  | ConstructionAction;

export type CameraAction = Exclude<InputAction, { readonly type: "select" } | ConstructionAction>;
