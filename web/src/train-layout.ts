/** The coaster models, by the keys the simulation uses for them. */
export const COASTER_MODEL_KEYS = ["hybrid", "velocity_viper", "wooden_twister"] as const;
export type CoasterModelKey = (typeof COASTER_MODEL_KEYS)[number];

/** A coaster's train, as the simulation configures it for its model. */
export interface TrainLayout {
  readonly model: CoasterModelKey;
  readonly carsPerTrain: number;
  readonly rowsPerCar: number;
  readonly seatsPerRow: number;
  readonly lengthMetres: number;
}

function isCoasterModelKey(value: string): value is CoasterModelKey {
  return (COASTER_MODEL_KEYS as readonly string[]).includes(value);
}

/** Reads the worker's tab-separated model key, cars, rows per car, seats per row and train length. */
export function parseTrainLayout(row: string): TrainLayout {
  const [model, ...numbers] = row.split("\t");
  if (!isCoasterModelKey(model) || numbers.length !== 4) throw new Error(`Invalid train layout "${row}"`);
  const [carsPerTrain, rowsPerCar, seatsPerRow, lengthMetres] = numbers.map(Number);
  if (![carsPerTrain, rowsPerCar, seatsPerRow].every((count) => Number.isInteger(count) && count > 0) || !(lengthMetres > 0)) {
    throw new Error(`Invalid train layout "${row}"`);
  }
  return { model, carsPerTrain, rowsPerCar, seatsPerRow, lengthMetres };
}
