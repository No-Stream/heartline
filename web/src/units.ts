export type Units = "imperial" | "metric";

const FEET_PER_METRE = 3.280839895013123;
const METRES_PER_FOOT = 0.3048;
const KILOMETRES_PER_HOUR_PER_METRE_PER_SECOND = 3.6;
const MILES_PER_HOUR_PER_METRE_PER_SECOND = 2.2369362920544;

export function metresToDisplay(metres: number, units: Units): number {
  return units === "imperial" ? metres * FEET_PER_METRE : metres;
}

export function displayToMetres(value: number, units: Units): number {
  return units === "imperial" ? value * METRES_PER_FOOT : value;
}

export function unitLabel(units: Units): "ft" | "m" {
  return units === "imperial" ? "ft" : "m";
}

export function formatLength(metres: number, units: Units, maximumFractionDigits = 1): string {
  const value = new Intl.NumberFormat("en-US", { maximumFractionDigits }).format(metresToDisplay(metres, units)).replace("-", "−");
  return `${value} ${unitLabel(units)}`;
}

export function metresPerSecondToDisplay(metresPerSecond: number, units: Units): number {
  return units === "imperial"
    ? metresPerSecond * MILES_PER_HOUR_PER_METRE_PER_SECOND
    : metresPerSecond * KILOMETRES_PER_HOUR_PER_METRE_PER_SECOND;
}

export function speedUnitLabel(units: Units): "mph" | "km/h" {
  return units === "imperial" ? "mph" : "km/h";
}

export function formatSpeed(metresPerSecond: number, units: Units, maximumFractionDigits = 1): string {
  const value = new Intl.NumberFormat("en-US", { maximumFractionDigits })
    .format(metresPerSecondToDisplay(metresPerSecond, units))
    .replace("-", "−");
  return `${value} ${speedUnitLabel(units)}`;
}
