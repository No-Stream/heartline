import { BRIDGE_LAYOUTS } from "./bridge-layouts";

export type TrainPieceKind = 0 | 1 | 2 | 3 | 4;

export interface TrainState {
  trainIndex: number;
  speed: number;
  pieceKind: TrainPieceKind;
  verticalG: number;
  lateralG: number;
  occupied: boolean;
}

const RECORD_SIZE = BRIDGE_LAYOUTS.TRAIN_STATE.length;

function visitTrainStates(records: Float32Array, visit?: (state: TrainState) => void): void {
  if (!(records instanceof Float32Array) || records.length % RECORD_SIZE !== 0) {
    throw new Error("Invalid train state snapshot");
  }

  const trainIndices = new Set<number>();
  for (let offset = 0; offset < records.length; offset += RECORD_SIZE) {
    const trainIndex = records[offset]!;
    const speed = records[offset + 1]!;
    const pieceKind = records[offset + 2]!;
    const verticalG = records[offset + 3]!;
    const lateralG = records[offset + 4]!;
    const occupied = records[offset + 5]!;
    if (
      !Number.isInteger(trainIndex) || trainIndex < 0 || trainIndices.has(trainIndex)
      || !Number.isFinite(speed) || speed < 0
      || !Number.isInteger(pieceKind) || pieceKind < 0 || pieceKind > 4
      || !Number.isFinite(verticalG) || !Number.isFinite(lateralG)
      || (occupied !== 0 && occupied !== 1)
    ) {
      throw new Error("Invalid train state snapshot");
    }
    trainIndices.add(trainIndex);
    visit?.({ trainIndex, speed, pieceKind: pieceKind as TrainPieceKind, verticalG, lateralG, occupied: occupied === 1 });
  }
}

export function validateTrainStates(records: Float32Array): void {
  visitTrainStates(records);
}

export function parseTrainStates(records: Float32Array): TrainState[] {
  const trainStates: TrainState[] = [];
  visitTrainStates(records, (state) => trainStates.push(state));
  return trainStates;
}
