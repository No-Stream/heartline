import type { ConstructionForceLimitsWire } from "./generated/ConstructionForceLimitsWire";
import type { TestingSample } from "./generated/TestingSample";
import { formatOneDecimal } from "./coaster-testing";
import { formatLength, formatSpeed, metresPerSecondToDisplay, speedUnitLabel, type Units } from "./units";

export const FORCE_AXES = ["verticalG", "lateralG", "longitudinalG"] as const;
export type ForceAxis = (typeof FORCE_AXES)[number];

export interface GraphBox {
  readonly width: number;
  readonly height: number;
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}

export interface GraphBand {
  readonly axis: ForceAxis;
  readonly top: number;
  readonly bottom: number;
}

export interface GraphTick {
  readonly value: number;
  readonly position: number;
  readonly label: string;
}

export interface RideGraph {
  readonly box: GraphBox;
  readonly plot: { readonly left: number; readonly right: number; readonly top: number; readonly bottom: number };
  readonly times: readonly number[];
  readonly durationSeconds: number;
  readonly forceRange: { readonly minimum: number; readonly maximum: number };
  readonly speedMaximum: number;
  readonly paths: Readonly<Record<ForceAxis | "speed", string>>;
  readonly bands: readonly GraphBand[];
  readonly timeTicks: readonly GraphTick[];
  readonly forceTicks: readonly GraphTick[];
  readonly speedTicks: readonly GraphTick[];
  timeToX(seconds: number): number;
  gToY(force: number): number;
  speedToY(metresPerSecond: number): number;
}

/** Ride time at each sample, using the same one-metre interval at the mean speed that the ride statistics use. */
export function sampleTimes(samples: readonly TestingSample[]): number[] {
  const times = [0];
  for (let index = 0; index + 1 < samples.length; index++) {
    const distance = samples[index + 1].distance > samples[index].distance ? samples[index + 1].distance - samples[index].distance : 1;
    const meanSpeed = (samples[index].speed + samples[index + 1].speed) / 2;
    times.push(times[index] + (meanSpeed > 0 ? distance / meanSpeed : 0));
  }
  return times;
}

const coordinate = (value: number): string => value.toFixed(1);

function niceStep(span: number, targetTicks: number): number {
  const rough = span / targetTicks;
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const fraction = rough / magnitude;
  return (fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10) * magnitude;
}

function ticksBetween(minimum: number, maximum: number, step: number): number[] {
  const first = Math.ceil(minimum / step - 1e-9);
  const last = Math.floor(maximum / step + 1e-9);
  return Array.from({ length: last - first + 1 }, (_, offset) => (first + offset) * step);
}

const minus = (text: string): string => text.replace("-", "−");

export function buildRideGraph(samples: readonly TestingSample[], limits: ConstructionForceLimitsWire | null, box: GraphBox, units: Units = "metric"): RideGraph {
  if (samples.length === 0) throw new Error("The ride graph has no samples");
  const plot = { left: box.left, right: box.width - box.right, top: box.top, bottom: box.height - box.bottom };
  const times = sampleTimes(samples);
  const durationSeconds = times[times.length - 1];
  const forces = samples.flatMap((sample) => [sample.verticalG, sample.lateralG, sample.longitudinalG]);
  const limitValues = limits
    ? [limits.minimumVerticalG, limits.maximumVerticalG, -limits.maximumLateralG, limits.maximumLateralG, -limits.maximumLongitudinalG, limits.maximumLongitudinalG]
    : [];
  const forceRange = {
    minimum: Math.floor(Math.min(0, ...forces, ...limitValues)),
    maximum: Math.ceil(Math.max(1, ...forces, ...limitValues)),
  };
  const speedMaximum = Math.max(1, ...samples.map((sample) => sample.speed));
  const timeToX = (seconds: number): number => plot.left + (durationSeconds > 0 ? seconds / durationSeconds : 0) * (plot.right - plot.left);
  const gToY = (force: number): number => plot.bottom - (force - forceRange.minimum) / (forceRange.maximum - forceRange.minimum) * (plot.bottom - plot.top);
  const speedToY = (metresPerSecond: number): number => plot.bottom - metresPerSecond / speedMaximum * (plot.bottom - plot.top);
  const line = (value: (sample: TestingSample) => number, toY: (value: number) => number): string =>
    samples.map((sample, index) => `${index === 0 ? "M" : "L"}${coordinate(timeToX(times[index]))} ${coordinate(toY(value(sample)))}`).join(" ");
  const bands: GraphBand[] = limits
    ? [
      { axis: "verticalG", top: gToY(limits.maximumVerticalG), bottom: gToY(limits.minimumVerticalG) },
      { axis: "lateralG", top: gToY(limits.maximumLateralG), bottom: gToY(-limits.maximumLateralG) },
      { axis: "longitudinalG", top: gToY(limits.maximumLongitudinalG), bottom: gToY(-limits.maximumLongitudinalG) },
    ]
    : [];
  const timeStep = niceStep(durationSeconds || 1, 6);
  const forceStep = forceRange.maximum - forceRange.minimum > 8 ? 2 : 1;
  const speedStepDisplay = niceStep(metresPerSecondToDisplay(speedMaximum, units), 4);
  const speedStep = speedStepDisplay / metresPerSecondToDisplay(1, units);
  return {
    box,
    plot,
    times,
    durationSeconds,
    forceRange,
    speedMaximum,
    paths: {
      verticalG: line((sample) => sample.verticalG, gToY),
      lateralG: line((sample) => sample.lateralG, gToY),
      longitudinalG: line((sample) => sample.longitudinalG, gToY),
      speed: line((sample) => sample.speed, speedToY),
    },
    bands,
    timeTicks: ticksBetween(0, durationSeconds, timeStep).map((value) => ({ value, position: timeToX(value), label: `${formatOneDecimal(value).replace(/\.0$/, "")} s` })),
    forceTicks: ticksBetween(forceRange.minimum, forceRange.maximum, forceStep).map((value) => ({ value, position: gToY(value), label: minus(`${value} g`) })),
    speedTicks: ticksBetween(0, speedMaximum, speedStep).map((value) => ({ value, position: speedToY(value), label: String(Math.round(metresPerSecondToDisplay(value, units))) })),
    timeToX,
    gToY,
    speedToY,
  };
}

/** The sample whose ride time is nearest to a pointer position given in graph coordinates. */
export function nearestSampleIndex(graph: RideGraph, x: number): number {
  const { plot, times, durationSeconds } = graph;
  const seconds = (Math.max(plot.left, Math.min(plot.right, x)) - plot.left) / (plot.right - plot.left) * durationSeconds;
  let low = 0;
  let high = times.length - 1;
  while (low < high) {
    const middle = (low + high) >> 1;
    if (times[middle] < seconds) low = middle + 1;
    else high = middle;
  }
  return low > 0 && seconds - times[low - 1] <= times[low] - seconds ? low - 1 : low;
}

export function speedAxisLabel(units: Units): string {
  return speedUnitLabel(units);
}

export function formatGraphReadout(sample: TestingSample, seconds: number, units: Units): string {
  return [
    `${formatOneDecimal(seconds)} s`,
    `Speed ${formatSpeed(sample.speed, units, 0)}`,
    `Vertical ${formatOneDecimal(sample.verticalG)} g`,
    `Lateral ${formatOneDecimal(sample.lateralG)} g`,
    `Forward ${formatOneDecimal(sample.longitudinalG)} g`,
    `${formatLength(sample.distance, units, 0)} along the track`,
  ].join(" · ");
}
