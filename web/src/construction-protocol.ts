import type { CoasterColorScheme } from "./coaster-colors";
import type { TestingReport } from "./coaster-testing";
import type { CoasterModelKey, TrainLayout } from "./train-layout";

/** Commands are processed in order by the simulation worker. */
export type ConstructionAction =
  | { readonly type: "construction-start"; readonly x: number; readonly y: number; readonly z: number; readonly heading: number }
  | { readonly type: "construction-select"; readonly piece: string }
  | { readonly type: "construction-set-chain"; readonly enabled: boolean }
  | { readonly type: "construction-append" }
  | { readonly type: "construction-undo" }
  | { readonly type: "construction-delete"; readonly id: number }
  | { readonly type: "construction-delete-mode"; readonly enabled: boolean }
  | { readonly type: "construction-close" }
  | { readonly type: "construction-finish-preview" }
  | { readonly type: "construction-finish-confirm" }
  | { readonly type: "construction-finish-cancel" }
  | { readonly type: "construction-set-open"; readonly open: boolean }
  | { readonly type: "construction-set-coaster-price"; readonly price: number }
  | { readonly type: "construction-set-colors"; readonly colors: CoasterColorScheme }
  | { readonly type: "construction-test" }
  | { readonly type: "construction-auto-blocks" }
  | { readonly type: "construction-apply-best-blocks" }
  | { readonly type: "construction-set-train-count"; readonly count: number }
  | { readonly type: "construction-move-block"; readonly index: number; readonly boundaryMetres: number }
  | { readonly type: "construction-remove-block"; readonly index: number }
  | { readonly type: "construction-add-block"; readonly boundaryMetres: number };

/** A coaster panel choice before the main thread translates it to the bridge's model selection. */
export type ConstructionPanelAction = { readonly type: "construction-set-model"; readonly model: CoasterModelKey };

export interface BlockSection {
  readonly boundaryMetres: number;
  readonly brakeStartMetres: number | null;
  readonly brakePieceIndices: readonly number[];
  readonly editable: boolean;
  readonly editableIndex: number | null;
  readonly reason: string;
  readonly trackKind: string;
}

export interface BlockReport {
  readonly sections: readonly BlockSection[];
  readonly trainCapacity: number;
  readonly capacitySearchComplete: boolean | null;
  readonly theoreticalHourlyRiders: number | null;
  readonly throughputStatus: string;
  readonly placementStatus: string;
  readonly placementSearchComplete: boolean | null;
  readonly reasons: readonly string[];
  readonly notes: readonly string[];
  readonly suggestions: readonly string[];
}

export interface ConstructionPiece {
  readonly id: string;
  readonly label: string;
  readonly group: string;
  readonly heading: string;
  readonly slope: string;
  readonly bank: string;
  /** Why the worker refuses this piece at the open end, or empty when it can be placed. */
  readonly refusal: string;
}

export interface ConstructionElement {
  readonly id: string;
  readonly name: string;
  readonly pieceId: string;
  readonly slope: string;
  readonly kind: string;
}

/** A catalog piece's centreline in metres, entering at the origin along +x, with the track's up vector at each point. */
export interface PieceShape {
  readonly id: string;
  readonly name: string;
  /** Track, Lift, Brakes or Station. */
  readonly kind: string;
  /** Heading change in eighth turns; positive turns left. */
  readonly turn: number;
  readonly entrySlope: string;
  readonly exitSlope: string;
  readonly entryBank: string;
  readonly exitBank: string;
  readonly points: readonly (readonly [number, number, number, number, number, number])[];
}

export interface PieceShapesMessage {
  readonly type: "piece-shapes";
  readonly shapes: readonly PieceShape[];
}

/** The worker is authoritative for offered pieces, preview, and placement validity. */
export interface ConstructionState {
  readonly type: "construction-state";
  readonly active: boolean;
  readonly chain: boolean;
  /** The coaster's paint, or null before a station is placed. */
  readonly colors: CoasterColorScheme | null;
  /** The coaster's train, or null before a station is placed. */
  readonly trainLayout: TrainLayout | null;
  readonly pieces: readonly ConstructionPiece[];
  readonly elements: readonly ConstructionElement[];
  readonly selected: string | null;
  readonly track: readonly {
    readonly id: number;
    /** Piece id used to find its icon geometry. */
    readonly label: string;
    readonly name: string;
    readonly group: string;
    readonly slope: string;
    readonly kind: string;
  }[];
  readonly canAppend: boolean;
  readonly canClose: boolean;
  readonly finishPreview: boolean;
  readonly canFinish: boolean;
  readonly finishReason: string;
  readonly canTest: boolean;
  readonly isOpen: boolean;
  readonly coasterPrice: number;
  readonly coasterIncome: bigint;
  readonly stationPositions: readonly number[];
  readonly message: string;
  readonly blockReport: BlockReport;
  readonly trainCount: number;
  readonly testingReport?: TestingReport | null;
  readonly placedRails: Float32Array;
  readonly railDistances: Float32Array;
  readonly blockCandidates: Float64Array;
  readonly placedTies: Float32Array;
  readonly placedSupports: Float32Array;
  readonly previewRails: Float32Array;
  readonly previewTies: Float32Array;
  readonly previewSupports: Float32Array;
}

export interface BlockAnalysisProgress {
  readonly type: "block-analysis-progress";
  readonly blockReport: BlockReport;
  readonly trainCount: number;
  readonly message: string;
}
