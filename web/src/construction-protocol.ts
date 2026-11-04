import type { CoasterColorScheme } from "./coaster-colors";
import type { TestingReport } from "./coaster-testing";

type ConstructionModel = "hybrid" | "velocity_viper" | "wooden_twister";

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
export type ConstructionPanelAction = { readonly type: "construction-set-model"; readonly model: ConstructionModel };

import type { BlockReport } from "./generated/BlockReport";
import type { PieceShapeReport as PieceShape } from "./generated/PieceShapeReport";
export type { BlockReport, PieceShape };
export type { BlockSectionReport as BlockSection } from "./generated/BlockSectionReport";

export type { ConstructionPieceWire as ConstructionPiece } from "./generated/ConstructionPieceWire";
export type { ConstructionElementWire as ConstructionElement } from "./generated/ConstructionElementWire";
export type { ConstructionTrackWire as ConstructionTrack } from "./generated/ConstructionTrackWire";
import type { ConstructionPieceWire as ConstructionPiece } from "./generated/ConstructionPieceWire";
import type { ConstructionElementWire as ConstructionElement } from "./generated/ConstructionElementWire";
import type { ConstructionTrackWire as ConstructionTrack } from "./generated/ConstructionTrackWire";

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
  readonly pieces: readonly ConstructionPiece[];
  readonly elements: readonly ConstructionElement[];
  readonly selected: string | null;
  readonly track: readonly ConstructionTrack[];
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
