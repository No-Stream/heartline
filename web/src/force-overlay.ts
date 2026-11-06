import type { ConstructionForcePreview } from "./construction-protocol";
import type { TestingSample } from "./generated/TestingSample";
import type { ScreenPoint } from "./input/actions";
import { formatSpeed, type Units } from "./units";

export type ForceLimits = ConstructionForcePreview["limits"];
export type LimitBand = "comfortable" | "near" | "past";

/** Limit closeness at or above this share of the model's comfortable limit is drawn as near the limit. */
export const NEAR_LIMIT_CLOSENESS = 0.75;
/** Hex colours for each band. They differ in lightness as well as hue, so they stay apart for colour-blind players. */
export const LIMIT_BAND_COLORS: Readonly<Record<LimitBand, string>> = { comfortable: "#4f8fd6", near: "#f2b53a", past: "#a3123a" };
export const LIMIT_BAND_LABELS: Readonly<Record<LimitBand, string>> = { comfortable: "Comfortable", near: "Near limit", past: "Past limit" };

function validateLimits(limits: ForceLimits): void {
  if (!(limits.minimumVerticalG < 0 && limits.maximumVerticalG > 0 && limits.maximumLateralG > 0 && limits.maximumLongitudinalG > 0)) {
    throw new Error("Invalid comfortable force limits");
  }
}

/** The worst of the three force axes as a share of the model's comfortable limit, so 1 sits exactly on a limit. */
export function limitCloseness(sample: TestingSample, limits: ForceLimits): number {
  validateLimits(limits);
  const vertical = sample.verticalG < 0 ? sample.verticalG / limits.minimumVerticalG : sample.verticalG / limits.maximumVerticalG;
  return Math.max(vertical, Math.abs(sample.lateralG) / limits.maximumLateralG, Math.abs(sample.longitudinalG) / limits.maximumLongitudinalG);
}

export function limitBand(closeness: number): LimitBand {
  return closeness > 1 ? "past" : closeness >= NEAR_LIMIT_CLOSENESS ? "near" : "comfortable";
}

export type ForceAxis = "airtime" | "vertical" | "lateral" | "longitudinal";
export type ForceRating = "weak" | "good" | "great" | "fine" | "strong" | "too much" | "dangerous";
/** Rates a force in g on one axis. Airtime takes the vertical g itself. */
export function rateForce(axis: ForceAxis, valueG: number, limits: ForceLimits): ForceRating {
  validateLimits(limits);
  const magnitude = axis === "airtime" ? -valueG : axis === "vertical" ? valueG : Math.abs(valueG);
  const band = limits.ratingBands[axis].find(({ upTo, inclusive }) => upTo === null || (inclusive ? magnitude <= upTo : magnitude < upTo));
  if (!band) throw new Error(`No force rating band covers ${valueG} g`);
  return band.rating;
}

/** Vertical force is rated as airtime below 0.3 g, where riders feel lifted rather than pressed in. */
const AIRTIME_BELOW_G = 0.3;
export function rateVertical(verticalG: number, limits: ForceLimits): { axis: "airtime" | "vertical"; rating: ForceRating } {
  const axis = verticalG < AIRTIME_BELOW_G ? "airtime" : "vertical";
  return { axis, rating: rateForce(axis, verticalG, limits) };
}

export type ForceLabelKind = "topSpeed" | "peakVertical" | "airtime" | "peakLateral" | "peakLongitudinal";
export interface ForceLabel {
  readonly kind: ForceLabelKind;
  readonly sampleIndex: number;
  readonly pastLimit: boolean;
  /** Whether the labelled spot lies on the last few placed pieces; older labels are drawn faded. */
  readonly recent: boolean;
}

/** The recent part of the track, measured back from the furthest point the estimated train reached. */
export const RECENT_TRACK_METRES = 30;

interface LabelAxis {
  readonly kind: ForceLabelKind;
  readonly score: (sample: TestingSample) => number;
  readonly notable: (sample: TestingSample, limits: ForceLimits) => boolean;
  readonly pastLimit: (sample: TestingSample, limits: ForceLimits) => boolean;
}
const labelAxes: readonly LabelAxis[] = [
  { kind: "topSpeed", score: (sample) => sample.speed, notable: (sample) => sample.speed > 0, pastLimit: () => false },
  { kind: "peakVertical", score: (sample) => sample.verticalG, notable: (sample, limits) => rateForce("vertical", sample.verticalG, limits) !== "weak", pastLimit: (sample, limits) => sample.verticalG > limits.maximumVerticalG },
  { kind: "airtime", score: (sample) => -sample.verticalG, notable: (sample) => sample.verticalG < AIRTIME_BELOW_G, pastLimit: (sample, limits) => sample.verticalG < limits.minimumVerticalG },
  { kind: "peakLateral", score: (sample) => Math.abs(sample.lateralG), notable: (sample, limits) => rateForce("lateral", sample.lateralG, limits) !== "fine", pastLimit: (sample, limits) => Math.abs(sample.lateralG) > limits.maximumLateralG },
  { kind: "peakLongitudinal", score: (sample) => Math.abs(sample.longitudinalG), notable: () => false, pastLimit: (sample, limits) => Math.abs(sample.longitudinalG) > limits.maximumLongitudinalG },
];

/**
 * Picks the few spots worth labelling: the worst point of every stretch past a comfortable limit anywhere on the
 * track, and the notable peaks of speed, vertical, airtime and lateral force on the recent part of the track.
 * Past-limit labels come first, most severe first, so they win when the labels do not all fit.
 */
export function selectForceLabels(samples: readonly TestingSample[], limits: ForceLimits): ForceLabel[] {
  validateLimits(limits);
  const last = samples.at(-1);
  if (!last) return [];
  const recentStart = last.distance - RECENT_TRACK_METRES;
  const isRecent = (index: number): boolean => samples[index].distance >= recentStart;
  const worstIndex = (from: number, to: number, axis: LabelAxis): number => {
    let worst = from;
    for (let index = from + 1; index <= to; index++) if (axis.score(samples[index]) > axis.score(samples[worst])) worst = index;
    return worst;
  };
  const pastLimitLabels: ForceLabel[] = [];
  const recentLabels: ForceLabel[] = [];
  for (const axis of labelAxes) {
    const pastStretches: [number, number][] = [];
    for (let index = 0; index < samples.length; index++) {
      if (!axis.pastLimit(samples[index], limits)) continue;
      const stretch = pastStretches.at(-1);
      if (stretch && stretch[1] === index - 1) stretch[1] = index;
      else pastStretches.push([index, index]);
    }
    for (const [from, to] of pastStretches) {
      const sampleIndex = worstIndex(from, to, axis);
      pastLimitLabels.push({ kind: axis.kind, sampleIndex, pastLimit: true, recent: isRecent(sampleIndex) });
    }
    const firstRecent = samples.findIndex((sample) => sample.distance >= recentStart);
    const peak = worstIndex(firstRecent, samples.length - 1, axis);
    const alreadyLabelled = pastStretches.some(([from, to]) => peak >= from && peak <= to);
    if (!alreadyLabelled && axis.notable(samples[peak], limits)) recentLabels.push({ kind: axis.kind, sampleIndex: peak, pastLimit: false, recent: true });
  }
  const severity = (label: ForceLabel): number => limitCloseness(samples[label.sampleIndex], limits);
  return [...pastLimitLabels.sort((first, second) => severity(second) - severity(first)), ...recentLabels];
}

const gText = (value: number): string => `${value.toFixed(1).replace("-", "−")} g`;

export function forceLabelText(label: ForceLabel, sample: TestingSample, limits: ForceLimits, units: Units): string {
  if (label.kind === "topSpeed") return formatSpeed(sample.speed, units, 0);
  if (label.kind === "peakVertical" || label.kind === "airtime") {
    const axis = label.kind === "airtime" ? "airtime" : "vertical";
    return `${gText(sample.verticalG)} · ${rateForce(axis, sample.verticalG, limits)} ${axis}`;
  }
  const axis = label.kind === "peakLateral" ? "lateral" : "longitudinal";
  const value = axis === "lateral" ? sample.lateralG : sample.longitudinalG;
  return `${gText(value)} · ${rateForce(axis, value, limits)} ${axis}`;
}

/** One line per value at a point of the estimate, each force with its rating. */
export function forceReadoutLines(sample: TestingSample, limits: ForceLimits, units: Units): { name: string; value: string }[] {
  const vertical = rateVertical(sample.verticalG, limits);
  return [
    { name: "Speed", value: formatSpeed(sample.speed, units) },
    { name: "Vertical", value: `${gText(sample.verticalG)} · ${vertical.rating}${vertical.axis === "airtime" ? " airtime" : ""}` },
    { name: "Lateral", value: `${gText(sample.lateralG)} · ${rateForce("lateral", sample.lateralG, limits)}` },
    { name: "Longitudinal", value: `${gText(sample.longitudinalG)} · ${rateForce("longitudinal", sample.longitudinalG, limits)}` },
  ];
}

export interface ScreenBox { readonly left: number; readonly top: number; readonly width: number; readonly height: number }

const LABEL_GAP = 9;
const LABEL_PADDING = 2;
const overlaps = (first: ScreenBox, second: ScreenBox): boolean =>
  first.left < second.left + second.width + LABEL_PADDING && second.left < first.left + first.width + LABEL_PADDING
  && first.top < second.top + second.height + LABEL_PADDING && second.top < first.top + first.height + LABEL_PADDING;
const containsPoint = (box: ScreenBox, point: ScreenPoint): boolean =>
  point.x > box.left - LABEL_PADDING && point.x < box.left + box.width + LABEL_PADDING && point.y > box.top - LABEL_PADDING && point.y < box.top + box.height + LABEL_PADDING;

/**
 * Places each label beside its anchor, in priority order: above, below, right or left. A label never overlaps an
 * earlier label, an obstacle, or the viewport edge; it avoids the drawn track when it can and is left out when nothing fits.
 */
export function placeForceLabels(
  labels: readonly { readonly anchor: ScreenPoint; readonly width: number; readonly height: number }[],
  viewport: { readonly width: number; readonly height: number },
  obstacles: readonly ScreenBox[],
  trackPoints: readonly ScreenPoint[],
): (ScreenBox | null)[] {
  const placed: ScreenBox[] = [...obstacles];
  return labels.map(({ anchor, width, height }) => {
    const candidates: ScreenBox[] = [
      { left: anchor.x - width / 2, top: anchor.y - LABEL_GAP - height, width, height },
      { left: anchor.x - width / 2, top: anchor.y + LABEL_GAP, width, height },
      { left: anchor.x + LABEL_GAP, top: anchor.y - height / 2, width, height },
      { left: anchor.x - LABEL_GAP - width, top: anchor.y - height / 2, width, height },
    ].filter((box) => box.left >= 0 && box.top >= 0 && box.left + box.width <= viewport.width && box.top + box.height <= viewport.height
      && !placed.some((other) => overlaps(box, other)));
    const choice = candidates.find((box) => !trackPoints.some((point) => containsPoint(box, point))) ?? candidates[0] ?? null;
    if (choice) placed.push(choice);
    return choice;
  });
}

/** The sample nearest a screen point, if the point lies within `maximumPixels` of it. */
export function nearestSampleIndex(points: readonly ScreenPoint[], target: ScreenPoint, maximumPixels: number): number | null {
  let nearest: number | null = null;
  let nearestDistance = maximumPixels;
  points.forEach((point, index) => {
    const distance = Math.hypot(point.x - target.x, point.y - target.y);
    if (distance <= nearestDistance) { nearest = index; nearestDistance = distance; }
  });
  return nearest;
}

/**
 * Labels the notable spots of a construction force estimate over the world canvas, and shows every value at the
 * hovered point. Positions are in CSS pixels; `render` runs each frame so the labels follow the camera.
 */
export function createForceOverlay(container: HTMLElement): {
  setEstimate(estimate: ConstructionForcePreview | null, units: Units): void;
  /** Returns whether the hovered sample changed. */
  setHover(sampleIndex: number | null): boolean;
  render(visible: boolean, project: (sample: TestingSample) => ScreenPoint, viewport: { width: number; height: number }, obstacles: readonly ScreenBox[]): void;
} {
  container.classList.add("force-overlay");
  const hoverMarker = document.createElement("div");
  hoverMarker.className = "force-overlay__marker";
  const hoverReadout = document.createElement("div");
  hoverReadout.className = "force-overlay__readout";
  hoverReadout.dataset.forceReadout = "";
  let estimate: ConstructionForcePreview | null = null;
  let labels: { label: ForceLabel; element: HTMLElement; dot: HTMLElement; width: number; height: number }[] = [];
  let hoverIndex: number | null = null;
  let activeUnits: Units = "imperial";
  const renderHoverText = (): void => {
    const sample = hoverIndex === null ? undefined : estimate?.samples[hoverIndex];
    if (!sample || !estimate) { hoverReadout.replaceChildren(); return; }
    const band = limitBand(limitCloseness(sample, estimate.limits));
    hoverReadout.dataset.band = band;
    hoverReadout.replaceChildren(...forceReadoutLines(sample, estimate.limits, activeUnits).flatMap(({ name, value }) => {
      const term = document.createElement("span");
      term.textContent = name;
      const description = document.createElement("strong");
      description.textContent = value;
      return [term, description];
    }));
  };
  return {
    setEstimate(value, units) {
      estimate = value;
      activeUnits = units;
      if (hoverIndex !== null && !value?.samples[hoverIndex]) hoverIndex = null;
      labels = value ? selectForceLabels(value.samples, value.limits).map((label) => {
        const element = document.createElement("div");
        element.className = "force-overlay__label";
        element.dataset.forceLabel = label.kind;
        element.dataset.pastLimit = String(label.pastLimit);
        element.dataset.recent = String(label.recent);
        element.textContent = forceLabelText(label, value.samples[label.sampleIndex], value.limits, units);
        const dot = document.createElement("div");
        dot.className = "force-overlay__dot";
        dot.dataset.pastLimit = String(label.pastLimit);
        dot.dataset.recent = String(label.recent);
        return { label, element, dot, width: 0, height: 0 };
      }) : [];
      container.replaceChildren(...labels.flatMap(({ dot, element }) => [dot, element]), hoverMarker, hoverReadout);
      renderHoverText();
    },
    setHover(sampleIndex) {
      if (sampleIndex === hoverIndex) return false;
      hoverIndex = sampleIndex;
      renderHoverText();
      return true;
    },
    render(visible, project, viewport, obstacles) {
      container.hidden = !visible || !estimate;
      if (container.hidden || !estimate) return;
      const samples = estimate.samples;
      const trackPoints = samples.map(project);
      for (const entry of labels) {
        if (entry.width === 0) { entry.width = entry.element.offsetWidth; entry.height = entry.element.offsetHeight; }
      }
      const boxes = placeForceLabels(labels.map(({ label, width, height }) => ({ anchor: trackPoints[label.sampleIndex], width, height })), viewport, obstacles, trackPoints);
      labels.forEach(({ label, element, dot }, index) => {
        const box = boxes[index];
        element.hidden = box === null;
        dot.hidden = box === null;
        if (!box) return;
        element.style.transform = `translate(${Math.round(box.left)}px, ${Math.round(box.top)}px)`;
        const anchor = trackPoints[label.sampleIndex];
        dot.style.transform = `translate(${Math.round(anchor.x)}px, ${Math.round(anchor.y)}px)`;
      });
      const hovered = hoverIndex === null ? undefined : trackPoints[hoverIndex];
      hoverMarker.hidden = !hovered;
      hoverReadout.hidden = !hovered;
      if (hovered) {
        hoverMarker.style.transform = `translate(${Math.round(hovered.x)}px, ${Math.round(hovered.y)}px)`;
        const width = hoverReadout.offsetWidth, height = hoverReadout.offsetHeight;
        const left = Math.min(Math.max(0, hovered.x + 14), viewport.width - width);
        const top = hovered.y - height - 14 >= 0 ? hovered.y - height - 14 : hovered.y + 14;
        hoverReadout.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
      }
    },
  };
}
