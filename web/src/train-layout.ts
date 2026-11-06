import type { TrainLayoutReport } from "./generated/TrainLayoutReport";
import { COASTER_MODELS, type CoasterModelKey } from "./generated/coaster-models";

export type { CoasterModelKey };
export type TrainLayout = Omit<TrainLayoutReport, "model"> & { readonly model: CoasterModelKey };

/** Validate the structured train layout returned by the simulation. */
export function parseTrainLayout(layout: TrainLayoutReport): TrainLayout {
  const model = COASTER_MODELS.find(({ key }) => key === layout.model);
  if (!model || model.track_material !== layout.trackMaterial ||
      ![layout.carsPerTrain, layout.rowsPerCar, layout.seatsPerRow].every((count) => Number.isInteger(count) && count > 0) ||
      !(layout.lengthMetres > 0)) {
    throw new Error(`Invalid train layout ${JSON.stringify(layout)}`);
  }
  return layout as TrainLayout;
}
