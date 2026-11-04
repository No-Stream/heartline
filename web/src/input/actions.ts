import type { ConstructionAction, ConstructionPanelAction } from "../construction-protocol";
import type { PieceStep } from "../construction-choices";
import type { DaySpeed } from "../day-clock";
import type { FacilityKind } from "../facilities";
import type { LandscapingSelection } from "../landscaping-tool";
import type { PathSelection } from "../path-panel";
import type { SceneryKind } from "../scenery-tool";
import type { TestingMetric } from "../coaster-testing";
import type { Theme } from "../theme";
import type { Tool } from "../toolbar";
import type { Units } from "../units";

export interface ScreenPoint {
  readonly x: number;
  readonly y: number;
}

/** Actions describe intent independently of the device that produced them. */
export type InputAction =
  | { readonly type: "pan"; readonly dx: number; readonly dy: number }
  | { readonly type: "zoom"; readonly factor: number; readonly anchor: ScreenPoint }
  | { readonly type: "rotate"; readonly turns: number }
  | { readonly type: "hover"; readonly position: ScreenPoint | null }
  | { readonly type: "station-placement"; readonly enabled: boolean }
  | { readonly type: "station-height"; readonly height: number }
  | { readonly type: "drag-start" | "drag-move" | "drag-end"; readonly position: ScreenPoint }
  | { readonly type: "drag-cancel" }
  | { readonly type: "block-marker-drag-start" | "block-marker-drag-move" | "block-marker-drag-end"; readonly position: ScreenPoint }
  | { readonly type: "block-marker-drag-cancel" | "block-marker-remove" }
  | { readonly type: "block-marker-nudge"; readonly step: -1 | 1 }
  | { readonly type: "path-build-mode" | "path-remove-mode" }
  | { readonly type: "select"; readonly position: ScreenPoint }
  /** Choose the next piece one step along an attribute, or through the whole palette. */
  | { readonly type: "piece-step"; readonly attribute: PieceStep; readonly step: -1 | 1 }
  | { readonly type: "piece-group"; readonly group: string }
  | { readonly type: "delete-mode-toggle" }
  | { readonly type: "chain-toggle" }
  | { readonly type: "game-speed-cycle" | "session-export" }
  | { readonly type: "day-scrub"; readonly phase: number }
  | { readonly type: "day-speed-set"; readonly speed: DaySpeed }
  | { readonly type: "testing-metric-set"; readonly metric: TestingMetric | null }
  | { readonly type: "theme-set"; readonly theme: Theme }
  | { readonly type: "units-set"; readonly units: Units }
  | { readonly type: "sound-mute-toggle" }
  | { readonly type: "sound-volume"; readonly volume: number }
  | { readonly type: "guest-select"; readonly guestId: string }
  | { readonly type: "guest-close" }
  | { readonly type: "landscaping-activate"; readonly selection: LandscapingSelection }
  | { readonly type: "landscaping-cancel" }
  | { readonly type: "underground-view-toggle" }
  | { readonly type: "underground-view-level"; readonly level: number }
  | { readonly type: "facility-place-mode"; readonly kind: FacilityKind; readonly rotation: number }
  | { readonly type: "facility-remove-mode" | "facility-cancel" }
  | { readonly type: "facility-price"; readonly id: number; readonly price: number }
  | { readonly type: "path-build-selection"; readonly selection: PathSelection }
  | { readonly type: "path-remove-selection" | "path-cancel" }
  | { readonly type: "scenery-place-mode"; readonly kind: SceneryKind; readonly rotation: number }
  | { readonly type: "scenery-remove-mode" | "scenery-cancel" }
  /** Open one tool panel, or none to watch the park, ending any other tool's input mode. */
  | { readonly type: "tool-select"; readonly tool: Tool }
  /** Leave the active input mode, or close the open tool when none is active. */
  | { readonly type: "builder-cancel" }
  | ConstructionAction
  | ConstructionPanelAction;

export type CameraAction = Extract<InputAction, { readonly type: "pan" | "zoom" | "rotate" }>;
