import type { TestingReport } from "./coaster-testing";

/** Commands are processed in order by the simulation worker. */
export type ConstructionAction =
  | { readonly type: "construction-start"; readonly x: number; readonly y: number; readonly z: number; readonly heading: number }
  | { readonly type: "construction-select"; readonly piece: string }
  | { readonly type: "construction-append" }
  | { readonly type: "construction-undo" }
  | { readonly type: "construction-delete"; readonly id: number }
  | { readonly type: "construction-delete-mode"; readonly enabled: boolean }
  | { readonly type: "construction-close" }
  | { readonly type: "construction-set-open"; readonly open: boolean }
  | { readonly type: "construction-test" };

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
  readonly pieces: readonly ConstructionPiece[];
  readonly selected: string | null;
  readonly track: readonly { readonly id: number; readonly label: string }[];
  readonly canAppend: boolean;
  readonly canClose: boolean;
  readonly canTest: boolean;
  readonly isOpen: boolean;
  readonly stationPositions: readonly number[];
  readonly message: string;
  readonly testingReport?: TestingReport | null;
  readonly placedRails: Float32Array;
  readonly placedTies: Float32Array;
  readonly placedSupports: Float32Array;
  readonly previewRails: Float32Array;
  readonly previewTies: Float32Array;
  readonly previewSupports: Float32Array;
}
