import type { Facility, FacilityAction } from "./facilities";
import type { ConstructionAction, ConstructionState, ConstructionFinishStatus, BlockAnalysisProgress, PieceShapesMessage } from "./construction-protocol";
import type { Scenery, SceneryCommand } from "./scenery-tool";
import type { TerrainEditCommand, TerrainEditResult } from "./landscaping-tool";
import type { PathToolCommand, PathToolMessage, PathPosition } from "./path-tool";
import type { SetSurfaceMessage, WorldSnapshot } from "./world-snapshot";
import type { GuestDetail } from "./guest-panel/protocol";
import type { RideInfo, RideKey } from "./ride-window/protocol";
import type { GuestSnapshot } from "./render/people";
import type { CoasterColorScheme } from "./coaster-colors";
import type { TrainLayout } from "./train-layout";
import type { WorkerRecording } from "./session-recording";

export type { PathPosition };
export const DEMO_COASTER_ID = 0xffff_ffff;
export type PathAction = PathToolCommand;
export type WorkerAction =
  | { type: "init"; seed: bigint; park: "new" | "demo"; guestCount: number; frozen: boolean; manualBlockAnalysis: boolean }
  | { type: "init"; seed: bigint; park: "description"; descriptionJson: string; guestCount: number; frozen: boolean; manualBlockAnalysis: boolean }
  | { type: "construction-input-barrier" }
  | { type: "export-session"; requestId: number }
  | { type: "save-park"; requestId: number }
  | { type: "load-park"; requestId: number; bytes: Uint8Array }
  | { type: "construction-finish-await" } | { type: "test-resume" } | { type: "guest-detail-close" }
  | { type: "set-speed"; speed: number }
  | { type: "set-paused"; paused: boolean }
  | SceneryCommand | SetSurfaceMessage | TerrainEditCommand | PathAction | ConstructionAction | FacilityAction
  | { type: "guest-detail-request"; guestId: string }
  | { type: "ride-info-request"; ride: RideKey } | { type: "ride-info-close" } | { type: "ride-test"; ride: RideKey }
  | { type: "facility-access-preview"; requestId: number; kind: Facility["kind"]; x: number; y: number; height: number; rotation: number }
  | { type: "facility-remove"; id: number }
  | { type: "test-advance"; count: number }
  | { type: "test-advance-block-analysis"; budget: number }
  | { type: "test-advance-block-placement"; budget: number }
  | { type: "facility-price"; id: number; price: number };

export interface CoasterRenderManifest {
  readonly id: number;
  readonly trackRevision: string;
  readonly colors: CoasterColorScheme | null;
  readonly trainLayout: TrainLayout | null;
}

export interface CoasterRenderData extends CoasterRenderManifest {
  readonly geometry: Float32Array;
  readonly rails: Float32Array;
  readonly ties: Float32Array;
  readonly supports: Float32Array;
}

export type CoasterWorldUpdate = CoasterRenderManifest & {
  readonly supports?: Float32Array;
  readonly geometry?: Float32Array;
  readonly rails?: Float32Array;
  readonly ties?: Float32Array;
};

export interface CoasterSnapshotData {
  readonly id: number;
  readonly trainPoses: Float32Array;
  readonly trainStates: Float32Array;
  readonly riders: Uint32Array;
}

export type CoasterWorldMessage = WorldSnapshot & {
  readonly editingCoasterId: number | null;
  readonly coasters: readonly CoasterWorldUpdate[];
};
export type CoasterSnapshot = GuestSnapshot & { tickCount: number; coasters: readonly CoasterSnapshotData[] };
export type WorkerMessage =
  | { type: "construction-input-barrier" }
  | { type: "session-export"; requestId: number; recording?: WorkerRecording; error?: string }
  | { type: "park-saved"; requestId: number; bytes?: Uint8Array; tickCount?: number; error?: string }
  | { type: "park-loaded"; requestId: number; error?: string }
  | { type: "initialization-error"; error: string }
  | { type: "park-restoring" }
  | CoasterWorldMessage | ConstructionState | ConstructionFinishStatus | BlockAnalysisProgress | ({ type: "snapshot"; sentAt: number } & CoasterSnapshot)
  | { type: "ready" } | { type: "test-advanced" } | { type: "test-block-analysis-advanced" } | { type: "test-resumed" } | { type: "construction-finish-ready" }
  | { type: "surface-edited"; changed: boolean }
  | { type: "facility-access-preview"; requestId: number; rotation: number | undefined }
  | { type: "facilities"; facilities: Facility[]; message: string; revenue: string; placed: boolean; forced: boolean; outcome: PlacementOutcome | null }
  | { type: "guest-detail"; guestId: string; detail: GuestDetail | null }
  /** The selected ride's latest test report; `testMessage` is why a requested test was refused, empty otherwise. */
  | { type: "ride-info"; info: RideInfo; testMessage: string }
  | PieceShapesMessage | PathToolMessage | TerrainEditResult | { type: "scenery"; scenery: Scenery[]; message: string; outcome: PlacementOutcome | null };

export function sendWorkerAction(worker: Worker, action: WorkerAction): void {
  worker.postMessage(action);
}

/** What a player's scenery or facility command did; `null` on updates that answer no such command. */
export type PlacementOutcome = "placed" | "removed" | "refused";

export function sendWorkerMessage(scope: { postMessage(message: WorkerMessage, transfer?: Transferable[]): void }, message: WorkerMessage, transfer?: Transferable[]): void {
  scope.postMessage(message, transfer);
}

export function assertNever(value: never): never {
  throw new Error(`Unhandled worker protocol message: ${JSON.stringify(value)}`);
}
