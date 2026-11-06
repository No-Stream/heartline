import { projectVertex } from "./render/projection";
import type { Camera, Viewport } from "./camera";
import { terrainHeightAt, type GeometryQuad, type Triangle, type Vertex, type WorldView } from "./render/scene";
import type { TrackStyle } from "./render/track/geometry";
import { formatLength, formatSpeed, speedUnitLabel, type Units } from "./units";
import type { InputAction } from "./input/actions";
import type { ConstructionForcePreview } from "./construction-protocol";
import { LIMIT_BAND_COLORS, LIMIT_BAND_LABELS, limitBand, limitCloseness, type LimitBand } from "./force-overlay";

import type { TestingSample } from "./generated/TestingSample";
import type { TestingReport } from "./generated/TestingReport";
import type { ScoreContributionReport } from "./generated/ScoreContributionReport";
import type { ProximityEventReport as ProximityEvent } from "./generated/ProximityEventReport";
import { findingDetails, findingEffects, findingHeadline, groupScoreFindings, scoreLabels, scoreNames, splitScoreFindings, type ScoreFinding, type ScoreName } from "./score-findings";
export type { TestingSample, TestingReport, ProximityEvent };
export type RideStatistics = TestingReport["statistics"];

export type TestingMetric = "limitCloseness" | SingleTestingMetric;
type SingleTestingMetric = "speed" | "verticalG" | "lateralG" | "longitudinalG" | "airtime";
export function overlaySamples(measured: readonly TestingSample[] | null, estimated: readonly TestingSample[] | null): readonly TestingSample[] {
  return measured ?? estimated ?? [];
}
const metrics: Record<SingleTestingMetric, { label: string; low: string; high: string; value(sample: TestingSample): number }> = {
  speed: { label: "Speed", low: "", high: "", value: (sample) => sample.speed / 30 },
  verticalG: { label: "Vertical force", low: "−1 g", high: "5 g+", value: (sample) => (sample.verticalG + 1) / 6 },
  lateralG: { label: "Lateral force", low: "0 g", high: "3 g+", value: (sample) => Math.abs(sample.lateralG) / 3 },
  longitudinalG: { label: "Longitudinal force", low: "0 g", high: "3 g+", value: (sample) => Math.abs(sample.longitudinalG) / 3 },
  airtime: { label: "Airtime", low: "No airtime", high: "Airtime", value: (sample) => Number(sample.airtime) },
};
function hexColor(hex: string): Triangle["color"] {
  const channel = (offset: number): number => parseInt(hex.slice(offset, offset + 2), 16) / 255;
  return [channel(1), channel(3), channel(5), 1];
}
function color(value: number, style: TrackStyle): Triangle["color"] {
  const amount = Math.max(0, Math.min(1, value));
  const channel = (hex: string, offset: number): number => parseInt(hex.slice(offset, offset + 2), 16) / 255;
  const mix = (offset: number): number => channel(style.train, offset) * (1 - amount) + channel(style.steel, offset) * amount;
  return [mix(1), mix(3), mix(5), 1];
}

function nearestTestingSegment(samples: readonly TestingSample[], distance: number): number | undefined {
  let nearestIndex: number | undefined;
  let nearestDistance = Infinity;
  for (let index = 0; index + 1 < samples.length; index++) {
    const startDistance = samples[index].distance;
    const endDistance = samples[index + 1].distance;
    if (endDistance <= startDistance || endDistance - startDistance > 1.001) continue;
    const distanceOnSegment = Math.max(startDistance, Math.min(endDistance, distance));
    const difference = Math.abs(distance - distanceOnSegment);
    if (difference < nearestDistance) {
      nearestIndex = index;
      nearestDistance = difference;
    }
  }
  return nearestIndex;
}

/** Colours the sampled track by one metric. Limit closeness needs the model's limits and outlines stretches past them in ink. */
export function buildTestingRibbon(
  samples: readonly TestingSample[],
  metric: TestingMetric,
  camera: Camera,
  viewport: Viewport,
  style: TrackStyle,
  selectedDistance?: number,
  limits?: ConstructionForcePreview["limits"],
): GeometryQuad[] {
  if (metric === "limitCloseness" && samples.length && !limits) throw new Error("The limit closeness overlay needs the model's comfortable force limits");
  const quads: GeometryQuad[] = [];
  const selectedIndex = selectedDistance === undefined ? undefined : nearestTestingSegment(samples, selectedDistance);
  const vertex = (sample: TestingSample): Vertex => projectVertex(camera, viewport, sample, 0.06);
  const exceedsLimit = (sample: TestingSample): boolean => !!limits && (
    metric === "verticalG" ? sample.verticalG < limits.minimumVerticalG || sample.verticalG > limits.maximumVerticalG
      : metric === "lateralG" ? Math.abs(sample.lateralG) > limits.maximumLateralG
        : metric === "longitudinalG" ? Math.abs(sample.longitudinalG) > limits.maximumLongitudinalG : false
  );
  const band = (start: TestingSample, end: TestingSample): LimitBand => limitBand(Math.max(limitCloseness(start, limits!), limitCloseness(end, limits!)));
  for (let index = 0; index + 1 < samples.length; index++) {
    const distanceDelta = samples[index + 1].distance - samples[index].distance;
    if (distanceDelta <= 0 || distanceDelta > 1.001) continue;
    const start = vertex(samples[index]);
    const end = vertex(samples[index + 1]);
    const dx = end.x - start.x, dy = end.y - start.y;
    const length = Math.hypot(dx, dy);
    if (length < 0.01) continue;
    const selected = index === selectedIndex;
    const strip = (halfWidth: number, stripColor: Triangle["color"], id: string): GeometryQuad => {
      const offsetX = -dy / length * halfWidth, offsetY = dx / length * halfWidth;
      return { vertices: [
        { ...start, x: start.x + offsetX, y: start.y + offsetY },
        { ...end, x: end.x + offsetX, y: end.y + offsetY },
        { ...end, x: end.x - offsetX, y: end.y - offsetY },
        { ...start, x: start.x - offsetX, y: start.y - offsetY },
      ], color: stripColor, kind: "testing", id, tileX: Math.floor(samples[index].x), tileY: Math.floor(samples[index].y) };
    };
    if (metric === "limitCloseness") {
      const segmentBand = band(samples[index], samples[index + 1]);
      if (selected || segmentBand === "past") quads.push(strip(selected ? 8 : 6, hexColor(style.ink), `testing-${index}`));
      quads.push(strip(segmentBand === "past" ? 4 : 3.5, hexColor(LIMIT_BAND_COLORS[segmentBand]), `testing-${index}`));
      continue;
    }
    const violation = exceedsLimit(samples[index]) || exceedsLimit(samples[index + 1]);
    if (selected) {
      quads.push(strip(8, hexColor(style.ink), `testing-${index}`));
      if (violation) quads.push(strip(4, [0.85, 0.1, 0.1, 1], `testing-${index}`));
    } else {
      quads.push(strip(4, violation ? [0.85, 0.1, 0.1, 1] : color(metrics[metric].value(samples[index]), style), `testing-${index}`));
    }
  }
  return quads;
}

const number = (value: number): string => value.toFixed(1).replace("-", "−");
const count = (value: number): string => Math.round(value).toLocaleString("en-US");
export function formatRideRating(rating: TestingReport["rating"]): string {
  return `Excitement: ${number(rating.excitement)} · Intensity: ${number(rating.intensity)} · Nausea: ${number(rating.nausea)}`;
}

export interface RideStatistic {
  readonly label: string;
  readonly value: string;
}

export interface FormattedRideStatistics {
  readonly airtime: readonly RideStatistic[];
  readonly forces: readonly RideStatistic[];
  readonly speedAndLayout: readonly RideStatistic[];
}

export function formatRideStatistics(statistics: RideStatistics, units: Units): FormattedRideStatistics {
  return {
    airtime: [
      { label: "Ejector airtime", value: `${number(statistics.ejectorAirtimeSeconds)} s` },
      { label: "Floater airtime", value: `${number(statistics.floaterAirtimeSeconds)} s` },
      { label: "Airtime moments", value: count(statistics.airtimeMoments) },
    ],
    forces: [
      { label: "Strong positive g", value: `${number(statistics.strongPositiveGSeconds)} s (longest ${number(statistics.longestStrongPositiveGSeconds)} s)` },
      { label: "Strong laterals", value: `${number(statistics.strongLateralGSeconds)} s` },
      { label: "Max positive vertical", value: `${number(statistics.maxPositiveVerticalG)} g` },
      { label: "Max negative vertical", value: `${number(statistics.maxNegativeVerticalG)} g` },
      { label: "Max lateral", value: `${number(statistics.maxLateralG)} g` },
      { label: "Max forward", value: `${number(statistics.maxForwardG)} g` },
      { label: "Max braking", value: `${number(statistics.maxBrakingG)} g` },
    ],
    speedAndLayout: [
      { label: "Max speed", value: formatSpeed(statistics.maxSpeed, units) },
      { label: "Average speed", value: formatSpeed(statistics.averageSpeed, units) },
      { label: "Ride duration", value: `${number(statistics.durationSeconds)} s` },
      { label: "Track length", value: formatLength(statistics.trackLength, units) },
      { label: "Max height", value: formatLength(statistics.maxHeight, units) },
      { label: "Biggest drop", value: formatLength(statistics.biggestDrop, units) },
      { label: "Steepest drop", value: `${number(statistics.steepestDropAngleDegrees)}°` },
      { label: "Inversions", value: `${count(statistics.inversions)} (${number(statistics.invertedSeconds)} s inverted)` },
      { label: "Drops", value: count(statistics.drops) },
      { label: "Launches", value: `${count(statistics.launches)} (peak ${statistics.launches ? formatSpeed(statistics.launchPeakSpeed, units) : "—"})` },
    ],
  };
}

const proximityLabels: Readonly<Record<string, string>> = {
  nearMiss: "Near-miss with the coaster's supports",
  headchopper: "A headchopper over the track",
  dueling: "Dueling trains",
  tunnel: "A fast tunnel pass",
  terrainHugging: "A low pass close to the ground",
};

export function formatProximityEvent(event: ProximityEvent): string {
  const label = proximityLabels[event.kind] ?? "A close coaster pass";
  const closeness = event.strength >= 0.75 ? "very close" : event.strength >= 0.4 ? "close" : "nearby";
  return `${label} at piece ${event.pieceIndex + 1}, ${closeness}`;
}

export function proximityContribution(events: readonly ProximityEvent[]): number {
  const strength = events.reduce((total, event) => total + Math.max(0, Math.min(1, event.strength)), 0);
  return strength === 0 ? 0 : 0.8 * strength / (strength + 4);
}

const proximityColors: Readonly<Record<string, "steel" | "wood" | "train" | "ink">> = {
  nearMiss: "steel", headchopper: "wood", dueling: "train", tunnel: "ink", terrainHugging: "wood",
};

export function buildProximityMarkers(events: readonly ProximityEvent[], camera: Camera, viewport: Viewport, style: TrackStyle): GeometryQuad[] {
  return events.flatMap((event, index) => {
    const center = projectVertex(camera, viewport, event, 0.12);
    const strength = Math.max(0, Math.min(1, event.strength));
    const halfSize = 4 + strength * 2;
    const vertex = (x: number, y: number): Vertex => ({ ...center, x, y });
    const marker = (size: number, hex: string): GeometryQuad => {
      const channel = (offset: number): number => parseInt(hex.slice(offset, offset + 2), 16) / 255;
      return {
        vertices: [vertex(center.x, center.y - size), vertex(center.x + size, center.y), vertex(center.x, center.y + size), vertex(center.x - size, center.y)],
        color: [channel(1), channel(3), channel(5), 1], kind: "testing", id: `proximity-${index}`,
        tileX: Math.floor(event.x), tileY: Math.floor(event.y),
      };
    };
    return [marker(halfSize + 2, style.ink), marker(halfSize - 1.5, style[proximityColors[event.kind] ?? "ink"])];
  });
}

export interface RideHeightSummary {
  readonly maximumHeightMetres: number;
  readonly biggestDropMetres: number;
}

export function summarizeRideHeights(samples: readonly TestingSample[], world: WorldView): RideHeightSummary | null {
  if (samples.length === 0) return null;
  let maximumHeightMetres = -Infinity;
  let highestElevation = samples[0].z * 4;
  let previousDistance = samples[0].distance;
  let biggestDropMetres = 0;
  for (const sample of samples) {
    const elevation = sample.z * 4;
    maximumHeightMetres = Math.max(maximumHeightMetres, elevation - terrainHeightAt(world, sample.x, sample.y));
  }
  for (const sample of samples.slice(1)) {
    const elevation = sample.z * 4;
    if (sample.distance - previousDistance > 1.001) highestElevation = elevation;
    biggestDropMetres = Math.max(biggestDropMetres, highestElevation - elevation);
    highestElevation = Math.max(highestElevation, elevation);
    previousDistance = sample.distance;
  }
  return { maximumHeightMetres, biggestDropMetres };
}

function formatSampleInUnits(sample: TestingSample, units: Units): string {
  return `Speed: ${formatSpeed(sample.speed, units)} · Vertical: ${number(sample.verticalG)} g · Lateral: ${number(sample.lateralG)} g · Longitudinal: ${number(sample.longitudinalG)} g · Airtime: ${sample.airtime ? "yes" : "no"}`;
}

export function formatSample(sample: TestingSample, units: Units = "imperial"): string {
  return formatSampleInUnits(sample, units);
}

export function formatConstructionEstimate(estimate: ConstructionForcePreview, units: Units): string {
  const { limits, samples } = estimate;
  const axesToCheck = [
    { name: "vertical", value: (sample: TestingSample) => sample.verticalG, excess: (value: number) => Math.max(limits.minimumVerticalG - value, value - limits.maximumVerticalG), limit: (value: number) => `limit ${number(value < limits.minimumVerticalG ? limits.minimumVerticalG : limits.maximumVerticalG)} g` },
    { name: "lateral", value: (sample: TestingSample) => sample.lateralG, excess: (value: number) => Math.abs(value) - limits.maximumLateralG, limit: () => `absolute limit ${number(limits.maximumLateralG)} g` },
    { name: "longitudinal", value: (sample: TestingSample) => sample.longitudinalG, excess: (value: number) => Math.abs(value) - limits.maximumLongitudinalG, limit: () => `absolute limit ${number(limits.maximumLongitudinalG)} g` },
  ];
  const violations = axesToCheck.flatMap((axis) => {
    const worst = samples.reduce<TestingSample | null>((current, sample) => axis.excess(axis.value(sample)) > (current ? axis.excess(axis.value(current)) : 0) ? sample : current, null);
    if (!worst) return [];
    const value = axis.value(worst);
    return [{ name: axis.name, detail: `${axis.name} ${number(value)} g at ${formatLength(worst.distance, units)} (${axis.limit(value)})` }];
  });
  const exceeded = violations.map((violation) => violation.name);
  const axes = exceeded.length === 3 ? `${exceeded[0]}, ${exceeded[1]} and ${exceeded[2]}` : exceeded.join(" and ");
  const warning = axes ? ` Estimated ${axes} forces exceed this model's comfortable limits: ${violations.map((violation) => violation.detail).join("; ")}.` : "";
  const last = samples.at(-1);
  const location = last ? ` Last recorded distance: ${formatLength(last.distance, units)}.` : " No distance was recorded.";
  const stopped = estimate.status === "stalled" ? ` The estimated train stopped.${location}` : estimate.status === "timedOut" ? ` The estimate timed out.${location}` : "";
  const peak = estimate.lastPlaced;
  const summary = peak ? `Last placed piece${estimate.completed ? "" : " (reached portion)"}: maximum speed ${formatSpeed(peak.maximumSpeed, units)} · Vertical ${number(peak.minimumVerticalG)}–${number(peak.maximumVerticalG)} g · Maximum lateral ${number(peak.maximumAbsoluteLateralG)} g · Maximum longitudinal ${number(peak.maximumAbsoluteLongitudinalG)} g.` : "The estimated train did not reach the last placed piece.";
  const launches = estimate.launches.map((launch) => {
    const force = launch.peakLongitudinalG === null ? "estimated force unavailable: no samples on this launch" : `configured launch's estimated peak longitudinal rider force ${number(launch.peakLongitudinalG)} g`;
    return ` Launch on piece ${launch.pieceIndex + 1}: approximate maximum speed ${formatSpeed(launch.maximumSpeed, units)} · ${force}.`;
  }).join("");
  return `Approximate construction estimate. ${summary}${launches}${warning}${stopped}`;
}

export function formatTestingNote(note: string, units: Units): string {
  const forceAdvice = /^(At .+ \()([\d.]+) m( along the track\), .+)$/.exec(note);
  if (forceAdvice) return `${forceAdvice[1]}${formatLength(Number(forceAdvice[2]), units)}${forceAdvice[3]}`;
  const contribution = /^(.+) \(([+−-][\d.]+), ([\d.]+)(?: ([A-Za-z0-9/-]+))?(?: at (.+), ([\d.]+) m)?\)$/.exec(note);
  if (contribution) {
    const [, label, score, rawMeasure, unit = "", pieceName, rawDistance] = contribution;
    const measure = Number(rawMeasure);
    const formattedMeasure = unit === "m/s"
      ? formatSpeed(measure, units)
      : unit === "m"
        ? formatLength(measure, units)
        : `${number(measure)}${unit ? ` ${unit}` : ""}`;
    const location = pieceName === undefined ? "" : ` at ${pieceName}, ${formatLength(Number(rawDistance), units)}`;
    return `${label} (${score}, ${formattedMeasure}${location})`;
  }
  const stalledDistance = /^The train could not finish the circuit after travelling ([\d.]+) m\.(.*)$/.exec(note);
  if (stalledDistance) {
    return `The train could not finish the circuit after travelling ${formatLength(Number(stalledDistance[1]), units)}.${stalledDistance[2]}`;
  }
  if (note === "Riders spend at least 8 m in a row under high forces. Add gentler sections between intense elements.") {
    return `Riders spend at least ${formatLength(8, units)} in a row under high forces. Add gentler sections between intense elements.`;
  }
  if (note === "This layout has at least 100 m in a row below 8 m/s. A taller first drop or a faster section could make it feel livelier.") {
    return `This layout has at least ${formatLength(100, units)} in a row below ${formatSpeed(8, units)}. A taller first drop or a faster section could make it feel livelier.`;
  }
  const ratedSpeed = /^(The train entered .+ at )([\d.]+) m\/s, (above|below)( its rated speed band of )([\d.]+)–([\d.]+) m\/s\.$/.exec(note);
  if (ratedSpeed) {
    const minimum = formatSpeed(Number(ratedSpeed[5]), units).replace(` ${speedUnitLabel(units)}`, "");
    return `${ratedSpeed[1]}${formatSpeed(Number(ratedSpeed[2]), units)}, ${ratedSpeed[3]}${ratedSpeed[4]}${minimum}–${formatSpeed(Number(ratedSpeed[6]), units)}.`;
  }
  if (/\b\d+(?:\.\d+)? m(?:\/s)?\b/.test(note)) throw new Error(`Ride testing note has an unformatted measurement: ${note}`);
  return note;
}

function legendBounds(metric: SingleTestingMetric, units: Units): { low: string; high: string } {
  if (metric === "speed") return { low: formatSpeed(0, units, 0), high: `${formatSpeed(30, units)}+` };
  return metrics[metric];
}

const lowerFirst = (text: string): string => text[0].toLowerCase() + text.slice(1);

const scoreTone = (score: ScoreName, value: number): "good" | "bad" | "neutral" =>
  score === "intensity" ? "neutral" : (value > 0) === (score === "excitement") ? "good" : "bad";

function effectChip(score: ScoreName, value: number): HTMLElement {
  const chip = document.createElement("span");
  chip.className = "testing-effect";
  chip.dataset.score = score;
  chip.dataset.tone = scoreTone(score, value);
  chip.textContent = `${scoreLabels[score]} ${value >= 0 ? "+" : ""}${number(value)}`;
  return chip;
}

/** The visually hidden separators keep the rating's text "Excitement: 3.3 · Intensity: 8.6 · Nausea: 5.5" for screen readers. */
function scoreTile(score: ScoreName, value: number, last: boolean): HTMLElement {
  const tile = document.createElement("div");
  tile.className = "testing-score";
  tile.dataset.score = score;
  tile.innerHTML = `<span class="testing-score__label"></span><span class="testing-sr">: </span><span class="testing-score__value"></span>${last ? "" : `<span class="testing-sr"> · </span>`}<span class="testing-score__meter" aria-hidden="true"><span></span></span>`;
  tile.querySelector<HTMLElement>(".testing-score__label")!.textContent = scoreLabels[score];
  tile.querySelector<HTMLElement>(".testing-score__value")!.textContent = number(value);
  tile.querySelector<HTMLElement>(".testing-score__meter > span")!.style.width = `${Math.max(0, Math.min(100, value * 10))}%`;
  return tile;
}

function statisticRow(statistic: RideStatistic): HTMLElement {
  const row = document.createElement("div");
  const label = document.createElement("dt");
  const separator = document.createElement("span");
  separator.className = "testing-sr";
  separator.textContent = ": ";
  label.append(statistic.label, separator);
  const value = document.createElement("dd");
  value.textContent = statistic.value;
  row.append(label, value);
  return row;
}

export function createTestingPanel(container: HTMLElement, style: TrackStyle, dispatch: (action: InputAction) => void, onScoreLocationChange: () => void): {
  update(report: TestingReport | null): void;
  updateEstimate(estimate: ConstructionForcePreview | null): void;
  setMetric(metric: TestingMetric | null): void;
  setUnits(units: Units): void;
  setWorld(world: WorldView): void;
  inspect(index: number): void;
  inspectProximity(index: number): void;
  selectedScoreLocation(): NonNullable<ScoreContributionReport["location"]> | undefined;
} {
  container.innerHTML = `<h3>Coaster testing</h3><p data-notes></p><ul data-launch-results aria-label="Launch exit speeds"></ul><section data-statistics hidden><h4>Scores (about 0–10)</h4>
    <div class="testing-scores" data-rating></div>
    <h5 class="testing-findings-heading">Biggest reasons</h5>
    <ol class="testing-findings" data-score-findings aria-label="Biggest reasons for these scores"></ol>
    <details class="testing-more" data-score-more><summary>More detail</summary>
    <ol class="testing-findings" data-score-findings-more aria-label="Smaller reasons"></ol>
    <p class="testing-note" data-score-unchanged></p>
    <p class="testing-note" data-score-averaging>Effects are averaged over all cars. Force numbers come from the car that felt them most; car 1 is the front car.</p></details>
    <div class="testing-statistics"><div><h4>Airtime</h4><dl data-statistics-airtime></dl></div><div><h4>Forces</h4><dl data-statistics-forces></dl></div>
    <div><h4>Speed and layout</h4><dl data-statistics-speed-layout></dl></div></div>
    <p class="testing-note" data-whole-train>Statistics cover the whole train using the worst car.</p></section>
    <p data-height-summary hidden>Maximum height above ground (sampled): <span data-report-maximum-height></span> · Biggest drop (sampled): <span data-report-drop-height></span></p>
    <p data-construction-estimate hidden></p><label>Track overlay <select data-metric><option value="off">Off</option><option value="limitCloseness">Limit closeness</option><option value="speed">Speed</option><option value="verticalG">Vertical force</option><option value="lateralG">Lateral force</option><option value="longitudinalG">Longitudinal force</option><option value="airtime">Airtime</option></select></label>
    <div class="testing-band-legend" data-band-legend hidden>${(["comfortable", "near", "past"] as const).map((band) => `<span data-band="${band}"><span class="testing-band-legend__swatch" aria-hidden="true" style="background:${LIMIT_BAND_COLORS[band]}"></span>${LIMIT_BAND_LABELS[band]}</span>`).join("")}</div>
    <div data-legend hidden><span data-low></span> <span data-gradient aria-hidden="true" style="display:inline-block;width:100px;height:10px"></span> <span data-high></span><span data-force-warning hidden> · <span aria-hidden="true" style="color:rgb(85% 10% 10%)">■</span> Above comfortable limit</span></div>
    <div data-proximity-events></div><p data-inspection>Click the coloured track to inspect it.</p>`;
  container.querySelector<HTMLElement>("[data-gradient]")!.style.background = `linear-gradient(to right, ${style.train}, ${style.steel})`;
  const select = container.querySelector<HTMLSelectElement>("[data-metric]")!;
  const legend = container.querySelector<HTMLElement>("[data-legend]")!;
  const bandLegend = container.querySelector<HTMLElement>("[data-band-legend]")!;
  const limitClosenessOption = select.querySelector<HTMLOptionElement>("option[value=limitCloseness]")!;
  const inspection = container.querySelector<HTMLElement>("[data-inspection]")!;
  let report: TestingReport | null = null;
  let estimate: ConstructionForcePreview | null = null;
  let autoSelectedEstimate = false;
  let world: WorldView | undefined;
  let activeUnits: Units = "imperial";
  let inspectedSampleIndex: number | undefined;
  let inspectedProximityIndex: number | undefined;
  let inspectedScoreLocation: ScoreContributionReport["location"];
  const renderInspection = (): void => {
    const samples = overlaySamples(report?.samples ?? null, estimate?.samples ?? null);
    if (inspectedSampleIndex !== undefined && samples[inspectedSampleIndex]) {
      inspection.textContent = formatSampleInUnits(samples[inspectedSampleIndex], activeUnits);
      return;
    }
    if (inspectedProximityIndex !== undefined && report?.proximityEvents[inspectedProximityIndex]) {
      const event = report.proximityEvents[inspectedProximityIndex];
      inspection.textContent = `${formatProximityEvent(event)} · Strength: ${number(event.strength)} · Distance: ${formatLength(event.distance, activeUnits)}`;
      return;
    }
    if (inspectedScoreLocation) {
      inspection.textContent = `Selected score location: ${inspectedScoreLocation.pieceName} · ${formatLength(inspectedScoreLocation.distance, activeUnits)} · ${number(inspectedScoreLocation.seconds)} s`;
      return;
    }
    inspection.textContent = "Click the coloured track to inspect it.";
  };
  const renderMetric = (): TestingMetric | null => {
    // Test-run reports carry no force limits, so limit closeness is offered only for the construction estimate.
    limitClosenessOption.hidden = limitClosenessOption.disabled = !!report;
    if (report && select.value === "limitCloseness") select.value = "speed";
    const metric = select.value === "off" ? null : select.value as TestingMetric;
    const samples = overlaySamples(report?.samples ?? null, estimate?.samples ?? null);
    legend.hidden = metric === null || metric === "limitCloseness" || !samples.length;
    bandLegend.hidden = metric !== "limitCloseness" || !samples.length;
    container.querySelector<HTMLElement>("[data-force-warning]")!.hidden = !!report || !estimate || (metric !== "verticalG" && metric !== "lateralG" && metric !== "longitudinalG");
    if (metric && metric !== "limitCloseness") {
      const bounds = legendBounds(metric, activeUnits);
      container.querySelector<HTMLElement>("[data-low]")!.textContent = bounds.low;
      container.querySelector<HTMLElement>("[data-high]")!.textContent = bounds.high;
    }
    return metric && samples.length ? metric : null;
  };
  select.addEventListener("change", () => dispatch({ type: "testing-metric-set", metric: renderMetric() }));
  const renderReport = (): void => {
    const notes = container.querySelector<HTMLElement>("[data-notes]")!;
    const rating = container.querySelector<HTMLElement>("[data-rating]")!;
    const statisticsSection = container.querySelector<HTMLElement>("[data-statistics]")!;
    const heightSummary = container.querySelector<HTMLElement>("[data-height-summary]")!;
    notes.textContent = report ? report.notes.map((note) => formatTestingNote(note, activeUnits)).join(" ") || "No concerns were noted during this test." : "Run a test to see the results.";
    statisticsSection.hidden = !report;
    rating.replaceChildren(...(report ? scoreNames.map((score, index) => scoreTile(score, report!.rating[score], index === scoreNames.length - 1)) : []));
    const statistics = report?.statistics;
    const formattedStatistics = statistics ? formatRideStatistics(statistics, activeUnits) : null;
    const renderStatistics = (selector: string, items: readonly RideStatistic[] | undefined): void => {
      container.querySelector<HTMLElement>(selector)!.replaceChildren(...(items ?? []).map(statisticRow));
    };
    renderStatistics("[data-statistics-airtime]", formattedStatistics?.airtime);
    renderStatistics("[data-statistics-forces]", formattedStatistics?.forces);
    renderStatistics("[data-statistics-speed-layout]", formattedStatistics?.speedAndLayout);
    const findings = splitScoreFindings(report ? groupScoreFindings(report.breakdown) : []);
    const findingRow = (finding: ScoreFinding): HTMLElement => {
      const headline = findingHeadline(finding, report!.statistics, activeUnits);
      const row = document.createElement("li");
      row.className = "testing-finding";
      row.dataset.scoreFinding = finding.key;
      const details = document.createElement("details");
      const summary = document.createElement("summary");
      const text = document.createElement("span");
      text.className = "testing-finding__text";
      const title = document.createElement("strong");
      title.textContent = headline.title;
      text.append(title);
      if (headline.location) {
        const location = headline.location;
        const button = document.createElement("button");
        button.type = "button";
        button.className = "testing-location";
        button.dataset.scoreLocation = finding.key;
        button.title = "Show this spot on the track";
        button.textContent = location.pieceName;
        button.setAttribute("aria-pressed", String(inspectedScoreLocation === location));
        button.addEventListener("click", (event) => {
          event.preventDefault();
          inspectedSampleIndex = undefined;
          inspectedProximityIndex = undefined;
          inspectedScoreLocation = location;
          for (const other of container.querySelectorAll<HTMLElement>("[data-score-location]")) other.setAttribute("aria-pressed", String(other === button));
          renderInspection();
          onScoreLocationChange();
          if (select.value === "off") dispatch({ type: "testing-metric-set", metric: "speed" });
        });
        text.append(" at ", button);
      }
      if (headline.measurement) text.append(`: ${headline.measurement}`);
      const effects = document.createElement("span");
      effects.className = "testing-finding__effects";
      effects.append(...findingEffects(finding).map(({ score, value }) => effectChip(score, value)));
      summary.append(text, effects);
      const numbers = document.createElement("ul");
      numbers.className = "testing-finding__details";
      numbers.append(...findingDetails(finding, activeUnits).map((line) => {
        const item = document.createElement("li");
        item.textContent = line;
        return item;
      }));
      details.append(summary, numbers);
      row.append(details);
      return row;
    };
    container.querySelector<HTMLElement>("[data-score-findings]")!.replaceChildren(...findings.main.map(findingRow));
    const moreFindings = container.querySelector<HTMLElement>("[data-score-findings-more]")!;
    moreFindings.replaceChildren(...findings.more.map(findingRow));
    moreFindings.hidden = findings.more.length === 0;
    const unchanged = container.querySelector<HTMLElement>("[data-score-unchanged]")!;
    unchanged.textContent = findings.unchanged.length
      ? `No effect on this ride: ${findings.unchanged.map((finding) => lowerFirst(findingHeadline(finding, report!.statistics, activeUnits).title)).join(", ")}.`
      : "";
    unchanged.hidden = findings.unchanged.length === 0;
    container.querySelector<HTMLElement>("[data-score-more] > summary")!.textContent = findings.more.length
      ? `More detail: ${findings.more.length} smaller ${findings.more.length === 1 ? "reason" : "reasons"}`
      : "More detail";
    const launchResults = container.querySelector<HTMLElement>("[data-launch-results]")!;
    launchResults.replaceChildren(...(report?.launches ?? []).map((launch) => {
      const item = document.createElement("li");
      item.textContent = `Launch ${launch.pieceIndex + 1} exit speed: ${formatSpeed(launch.exitSpeed, activeUnits)}`;
      return item;
    }));
    launchResults.hidden = launchResults.childElementCount === 0;
    let summary: RideHeightSummary | null = null;
    if (report?.samples.length) {
      if (!world) throw new Error("Ride testing report requires a world snapshot");
      summary = summarizeRideHeights(report.samples, world);
    }
    heightSummary.hidden = summary === null;
    container.querySelector<HTMLElement>("[data-report-maximum-height]")!.textContent = summary ? formatLength(summary.maximumHeightMetres, activeUnits) : "";
    container.querySelector<HTMLElement>("[data-report-drop-height]")!.textContent = summary ? formatLength(summary.biggestDropMetres, activeUnits) : "";
    const eventList = container.querySelector<HTMLElement>("[data-proximity-events]")!;
    eventList.replaceChildren();
    if (report?.proximityEvents.length) {
      const heading = document.createElement("h4");
      heading.textContent = `Proximity events (${report.proximityEvents.length})`;
      eventList.append(heading);
      const groups = new Map<string, ProximityEvent[]>();
      for (const event of report.proximityEvents) groups.set(event.kind, [...(groups.get(event.kind) ?? []), event]);
      for (const [kind, events] of groups) {
        const section = document.createElement("section");
        const title = document.createElement("h5");
        title.textContent = `${proximityLabels[kind] ?? "Other close passes"} (${events.length})`;
        section.append(title);
        const list = document.createElement("ul");
        for (const event of events) {
          const item = document.createElement("li");
          item.textContent = formatProximityEvent(event);
          list.append(item);
        }
        section.append(list);
        eventList.append(section);
      }
      const contribution = document.createElement("p");
      contribution.textContent = `Added ${number(proximityContribution(report.proximityEvents))} excitement from proximity events.`;
      eventList.append(contribution);
    }
    const estimateText = container.querySelector<HTMLElement>("[data-construction-estimate]")!;
    estimateText.hidden = !estimate || !!report;
    estimateText.textContent = estimate && !report ? formatConstructionEstimate(estimate, activeUnits) : "";
    select.disabled = !overlaySamples(report?.samples ?? null, estimate?.samples ?? null).length;
    renderInspection();
    renderMetric();
  };
  return {
    update(value) {
      report = value; inspectedSampleIndex = undefined; inspectedProximityIndex = undefined;
      inspectedScoreLocation = undefined;
      onScoreLocationChange();
      const metricBefore = select.value;
      renderReport();
      if (select.value !== metricBefore) dispatch({ type: "testing-metric-set", metric: renderMetric() });
    },
    updateEstimate(value) {
      estimate = value;
      inspectedSampleIndex = undefined;
      if (!value) autoSelectedEstimate = false;
      if (value?.samples.length && !report && !autoSelectedEstimate) {
        autoSelectedEstimate = true;
        if (select.value === "off") { select.value = "limitCloseness"; dispatch({ type: "testing-metric-set", metric: "limitCloseness" }); }
      }
      renderReport();
    },
    setMetric(metric) { select.value = metric ?? "off"; renderMetric(); },
    setUnits(units) { activeUnits = units; renderReport(); },
    setWorld(value) { world = value; renderReport(); },
    inspect(index) {
      if (overlaySamples(report?.samples ?? null, estimate?.samples ?? null)[index]) {
        inspectedSampleIndex = index;
        inspectedProximityIndex = undefined;
        inspectedScoreLocation = undefined;
        renderInspection();
        onScoreLocationChange();
      }
    },
    inspectProximity(index) {
      if (report?.proximityEvents[index]) {
        inspectedProximityIndex = index;
        inspectedSampleIndex = undefined;
        inspectedScoreLocation = undefined;
        renderInspection();
        onScoreLocationChange();
      }
    },
    selectedScoreLocation() { return inspectedScoreLocation; },
  };
}
