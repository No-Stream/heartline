import type { TrainLayoutReport } from "./generated/TrainLayoutReport";
import type { TrainModelKey } from "./generated/TrainModelKey";

export type CoasterModelKey = TrainModelKey;
export type TrainLayout = TrainLayoutReport;

/** Validate the structured train layout returned by the simulation. */
export function parseTrainLayout(layout: TrainLayoutReport): TrainLayout {
  if (!(layout.model === "hybrid" || layout.model === "velocity_viper" || layout.model === "wooden_twister") ||
      ![layout.carsPerTrain, layout.rowsPerCar, layout.seatsPerRow].every((count) => Number.isInteger(count) && count > 0) ||
      !(layout.lengthMetres > 0)) {
    throw new Error(`Invalid train layout ${JSON.stringify(layout)}`);
  }
  return layout;
}
