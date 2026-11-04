import { projectVertex } from "./render/projection";
import type { Camera, Viewport } from "./camera";
import { terrainHeightAt, type GeometryQuad, type Triangle, type Vertex, type WorldView } from "./render/scene";
import type { TrackStyle } from "./render/track/geometry";
import { formatLength, formatSpeed, type Units } from "./units";
import type { InputAction } from "./input/actions";

import type { TestingSample } from "./generated/TestingSample";
import type { TestingReport } from "./generated/TestingReport";
import type { ProximityEventReport as ProximityEvent } from "./generated/ProximityEventReport";
export type { TestingSample, TestingReport, ProximityEvent };

export type TestingMetric = "speed" | "verticalG" | "lateralG" | "longitudinalG" | "airtime";
const metrics: Record<TestingMetric, { label: string; low: string; high: string; value(sample: TestingSample): number }> = {
  speed: { label: "Speed", low: "", high: "", value: (sample) => sample.speed / 30 },
  verticalG: { label: "Vertical force", low: "−1 g", high: "5 g+", value: (sample) => (sample.verticalG + 1) / 6 },
  lateralG: { label: "Lateral force", low: "0 g", high: "3 g+", value: (sample) => Math.abs(sample.lateralG) / 3 },
  longitudinalG: { label: "Longitudinal force", low: "0 g", high: "3 g+", value: (sample) => Math.abs(sample.longitudinalG) / 3 },
  airtime: { label: "Airtime", low: "No airtime", high: "Airtime", value: (sample) => Number(sample.airtime) },
};
function color(value: number, style: TrackStyle): Triangle["color"] {
  const amount = Math.max(0, Math.min(1, value));
  const channel = (hex: string, offset: number): number => parseInt(hex.slice(offset, offset + 2), 16) / 255;
  const mix = (offset: number): number => channel(style.train, offset) * (1 - amount) + channel(style.steel, offset) * amount;
  return [mix(1), mix(3), mix(5), 1];
}

export function buildTestingRibbon(samples: readonly TestingSample[], metric: TestingMetric, camera: Camera, viewport: Viewport, style: TrackStyle): GeometryQuad[] {
  const quads: GeometryQuad[] = [];
  const vertex = (sample: TestingSample): Vertex => projectVertex(camera, viewport, sample, 0.06);
  for (let index = 0; index + 1 < samples.length; index++) {
    if (samples[index + 1].distance - samples[index].distance > 1.001) continue;
    const start = vertex(samples[index]);
    const end = vertex(samples[index + 1]);
    const dx = end.x - start.x, dy = end.y - start.y;
    const length = Math.hypot(dx, dy);
    if (length < 0.01) continue;
    const offsetX = -dy / length * 4, offsetY = dx / length * 4;
    quads.push({ vertices: [
      { ...start, x: start.x + offsetX, y: start.y + offsetY },
      { ...end, x: end.x + offsetX, y: end.y + offsetY },
      { ...end, x: end.x - offsetX, y: end.y - offsetY },
      { ...start, x: start.x - offsetX, y: start.y - offsetY },
    ], color: color(metrics[metric].value(samples[index]), style), kind: "testing", id: `testing-${index}`,
    tileX: Math.floor(samples[index].x), tileY: Math.floor(samples[index].y) });
  }
  return quads;
}

const number = (value: number): string => value.toFixed(1).replace("-", "−");
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
  return strength === 0 ? 0 : 8 * strength / (strength + 4);
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

export function formatTestingNote(note: string, units: Units): string {
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
  if (/\b\d+(?:\.\d+)? m(?:\/s)?\b/.test(note)) throw new Error(`Ride testing note has an unformatted measurement: ${note}`);
  return note;
}

function legendBounds(metric: TestingMetric, units: Units): { low: string; high: string } {
  if (metric === "speed") return { low: formatSpeed(0, units, 0), high: `${formatSpeed(30, units)}+` };
  return metrics[metric];
}

export function createTestingPanel(container: HTMLElement, style: TrackStyle, dispatch: (action: InputAction) => void): {
  update(report: TestingReport | null): void;
  setMetric(metric: TestingMetric | null): void;
  setUnits(units: Units): void;
  setWorld(world: WorldView): void;
  inspect(index: number): void;
  inspectProximity(index: number): void;
} {
  container.innerHTML = `<h3>Coaster testing</h3><p data-notes></p><p data-rating></p><ul data-launch-results aria-label="Launch exit speeds"></ul>
    <p data-height-summary hidden>Maximum height above ground (sampled): <span data-report-maximum-height></span> · Biggest drop (sampled): <span data-report-drop-height></span></p>
    <label>Track overlay <select data-metric><option value="off">Off</option><option value="speed">Speed</option><option value="verticalG">Vertical force</option><option value="lateralG">Lateral force</option><option value="longitudinalG">Longitudinal force</option><option value="airtime">Airtime</option></select></label>
    <div data-legend hidden><span data-low></span> <span data-gradient aria-hidden="true" style="display:inline-block;width:100px;height:10px"></span> <span data-high></span></div>
    <div data-proximity-events></div><p data-inspection>Click the coloured track to inspect it.</p>`;
  container.querySelector<HTMLElement>("[data-gradient]")!.style.background = `linear-gradient(to right, ${style.train}, ${style.steel})`;
  const select = container.querySelector<HTMLSelectElement>("[data-metric]")!;
  const legend = container.querySelector<HTMLElement>("[data-legend]")!;
  const inspection = container.querySelector<HTMLElement>("[data-inspection]")!;
  let report: TestingReport | null = null;
  let world: WorldView | undefined;
  let activeUnits: Units = "imperial";
  let inspectedSampleIndex: number | undefined;
  let inspectedProximityIndex: number | undefined;
  const renderInspection = (): void => {
    if (inspectedSampleIndex !== undefined && report?.samples[inspectedSampleIndex]) {
      inspection.textContent = formatSampleInUnits(report.samples[inspectedSampleIndex], activeUnits);
      return;
    }
    if (inspectedProximityIndex !== undefined && report?.proximityEvents[inspectedProximityIndex]) {
      const event = report.proximityEvents[inspectedProximityIndex];
      inspection.textContent = `${formatProximityEvent(event)} · Strength: ${number(event.strength)} · Distance: ${formatLength(event.distance, activeUnits)}`;
      return;
    }
    inspection.textContent = "Click the coloured track to inspect it.";
  };
  const renderMetric = (): TestingMetric | null => {
    const metric = select.value === "off" ? null : select.value as TestingMetric;
    legend.hidden = metric === null || !report?.samples.length;
    if (metric) {
      const bounds = legendBounds(metric, activeUnits);
      container.querySelector<HTMLElement>("[data-low]")!.textContent = bounds.low;
      container.querySelector<HTMLElement>("[data-high]")!.textContent = bounds.high;
    }
    return metric && report?.samples.length ? metric : null;
  };
  select.addEventListener("change", () => dispatch({ type: "testing-metric-set", metric: renderMetric() }));
  const renderReport = (): void => {
    const notes = container.querySelector<HTMLElement>("[data-notes]")!;
    const rating = container.querySelector<HTMLElement>("[data-rating]")!;
    const heightSummary = container.querySelector<HTMLElement>("[data-height-summary]")!;
    notes.textContent = report ? report.notes.map((note) => formatTestingNote(note, activeUnits)).join(" ") || "No concerns were noted during this test." : "Run a test to see the results.";
    rating.textContent = report?.rating ? `Excitement: ${number(report.rating.excitement)} · Intensity: ${number(report.rating.intensity)} · Nausea: ${number(report.rating.nausea)}` : "";
    const launchResults = container.querySelector<HTMLElement>("[data-launch-results]")!;
    launchResults.replaceChildren(...(report?.launches ?? []).map((launch) => {
      const item = document.createElement("li");
      item.textContent = `Launch ${launch.pieceIndex + 1} exit speed: ${formatSpeed(launch.exitSpeed, activeUnits)}`;
      return item;
    }));
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
    select.disabled = !report?.samples.length;
    renderInspection();
    renderMetric();
  };
  return {
    update(value) { report = value; inspectedSampleIndex = undefined; inspectedProximityIndex = undefined; renderReport(); },
    setMetric(metric) { select.value = metric ?? "off"; renderMetric(); },
    setUnits(units) { activeUnits = units; renderReport(); },
    setWorld(value) { world = value; renderReport(); },
    inspect(index) {
      if (report?.samples[index]) {
        inspectedSampleIndex = index;
        inspectedProximityIndex = undefined;
        renderInspection();
      }
    },
    inspectProximity(index) {
      if (report?.proximityEvents[index]) {
        inspectedProximityIndex = index;
        inspectedSampleIndex = undefined;
        renderInspection();
      }
    },
  };
}
