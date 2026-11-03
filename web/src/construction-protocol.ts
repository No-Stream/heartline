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
  | { readonly type: "construction-test" };

export interface ConstructionPiece {
  readonly id: string;
  readonly label: string;
  readonly group: string;
  readonly heading: string;
  readonly slope: string;
  readonly bank: string;
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
  readonly message: string;
  readonly testingReport?: TestingReport | null;
  readonly placedRails: Float32Array;
  readonly placedTies: Float32Array;
  readonly placedSupports: Float32Array;
  readonly previewRails: Float32Array;
  readonly previewTies: Float32Array;
  readonly previewSupports: Float32Array;
}
