import type { Facility, FacilityAction } from "./facilities";
import type { ConstructionAction, ConstructionState, BlockAnalysisProgress, PieceShapesMessage } from "./construction-protocol";
import type { Scenery, SceneryCommand } from "./scenery-tool";
import type { TerrainEditCommand, TerrainEditResult } from "./landscaping-tool";
import type { PathToolCommand, PathToolMessage, PathPosition } from "./path-tool";
import type { SetSurfaceMessage, WorldSnapshot } from "./world-snapshot";
import type { GuestDetail } from "./guest-panel/protocol";
import type { GuestSnapshot } from "./render/people";
import type { CoasterColorScheme } from "./coaster-colors";
import type { WorkerRecording } from "./session-recording";

export type { PathPosition };
export type PathAction = PathToolCommand;
export type WorkerAction =
  | { type: "init"; seed: bigint; park: "new" | "demo"; guestCount: number; frozen: boolean; manualBlockAnalysis: boolean }
  | { type: "export-session"; requestId: number }
  | { type: "construction-finish-await" } | { type: "test-resume" } | { type: "guest-detail-close" }
  | { type: "set-speed"; speed: number }
  | SceneryCommand | SetSurfaceMessage | TerrainEditCommand | PathAction | ConstructionAction | FacilityAction
  | { type: "guest-detail-request"; guestId: string }
  | { type: "facility-access-preview"; requestId: number; kind: Facility["kind"]; x: number; y: number; height: number; rotation: number }
  | { type: "facility-remove"; id: number }
  | { type: "test-advance"; count: number }
  | { type: "test-advance-block-analysis"; budget: number }
  | { type: "facility-price"; id: number; price: number };

export type CoasterWorldMessage = WorldSnapshot & { coasterGeometry?: Float32Array; coasterRails?: Float32Array; coasterTies?: Float32Array; coasterSupports: Float32Array; coasterColors?: CoasterColorScheme | null };
export type CoasterSnapshot = GuestSnapshot & { tickCount: number; trainPoses: Float32Array; constructionTrainPoses: Float32Array; constructionRiders: Uint32Array };
export type WorkerMessage =
  | { type: "session-export"; requestId: number; recording: WorkerRecording }
  | CoasterWorldMessage | ConstructionState | BlockAnalysisProgress | ({ type: "snapshot"; sentAt: number } & CoasterSnapshot)
  | { type: "ready" } | { type: "test-advanced" } | { type: "test-block-analysis-advanced" } | { type: "test-resumed" } | { type: "construction-finish-ready" }
  | { type: "surface-edited"; changed: boolean }
  | { type: "facility-access-preview"; requestId: number; rotation: number | undefined }
  | { type: "facilities"; facilities: Facility[]; message: string; revenue: string; placed: boolean; forced: boolean }
  | { type: "guest-detail"; guestId: string; detail: GuestDetail | null }
  | PieceShapesMessage | PathToolMessage | TerrainEditResult | { type: "scenery"; scenery: Scenery[]; message: string };

export function sendWorkerAction(worker: Worker, action: WorkerAction): void {
  worker.postMessage(action);
}

export function sendWorkerMessage(scope: { postMessage(message: WorkerMessage, transfer?: Transferable[]): void }, message: WorkerMessage, transfer?: Transferable[]): void {
  scope.postMessage(message, transfer);
}

export function assertNever(value: never): never {
  throw new Error(`Unhandled worker protocol message: ${JSON.stringify(value)}`);
}
