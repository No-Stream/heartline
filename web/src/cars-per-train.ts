export function carsPerTrainControl(cars: number, minimum: number, maximum: number, seatsPerCar: number, unavailable: boolean): {
  capacityText: string;
  fewerDisabled: boolean;
  moreDisabled: boolean;
} {
  const capacity = cars * seatsPerCar;
  return {
    capacityText: `${capacity} ${capacity === 1 ? "rider" : "riders"} per train`,
    fewerDisabled: unavailable || cars <= minimum,
    moreDisabled: unavailable || cars >= maximum,
  };
}
