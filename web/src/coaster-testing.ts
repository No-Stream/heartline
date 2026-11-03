import { projectWorld, type Camera, type Viewport } from "./camera";
import type { GeometryQuad, Triangle, Vertex } from "./render/scene";
import type { TrackStyle } from "./render/track/geometry";

export interface TestingSample {
  distance: number; x: number; y: number; z: number; speed: number;
  verticalG: number; lateralG: number; longitudinalG: number; airtime: boolean;
}
export interface TestingReport {
  samples: TestingSample[];
  notes: string[];
  proximityEvents: ProximityEvent[];
  rating: { excitement: number; intensity: number; nausea: number } | null;
}
export interface ProximityEvent {
  kind: string; pieceIndex: number; distance: number; x: number; y: number; z: number; strength: number;
}
export type TestingMetric = "speed" | "verticalG" | "lateralG" | "longitudinalG" | "airtime";
const metrics: Record<TestingMetric, { label: string; low: string; high: string; value(sample: TestingSample): number }> = {
  speed: { label: "Speed", low: "0 m/s", high: "30 m/s+", value: (sample) => sample.speed / 30 },
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
  const vertex = (sample: TestingSample): Vertex => ({ ...projectWorld(camera, viewport, sample),
    depth: [sample.x + sample.y, sample.x - sample.y, -sample.x - sample.y, -sample.x + sample.y][camera.rotation] + sample.z + 0.06,
    height: sample.z * 4, u: 0, v: 0 });
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
    const center = projectWorld(camera, viewport, event);
    const strength = Math.max(0, Math.min(1, event.strength));
    const halfSize = 4 + strength * 2;
    const depth = [event.x + event.y, event.x - event.y, -event.x - event.y, -event.x + event.y][camera.rotation] + event.z + 0.12;
    const vertex = (x: number, y: number): Vertex => ({ ...center, x, y, depth, height: event.z * 4, u: 0, v: 0 });
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

export function formatSample(sample: TestingSample): string {
  return `Speed: ${number(sample.speed)} m/s · Vertical: ${number(sample.verticalG)} g · Lateral: ${number(sample.lateralG)} g · Longitudinal: ${number(sample.longitudinalG)} g · Airtime: ${sample.airtime ? "yes" : "no"}`;
}

export function createTestingPanel(container: HTMLElement, style: TrackStyle, onChange: (metric: TestingMetric | null) => void): { update(report: TestingReport | null): void; inspect(index: number): void; inspectProximity(index: number): void } {
  container.innerHTML = `<h3>Coaster testing</h3><p data-notes></p><p data-rating></p>
    <label>Track overlay <select data-metric><option value="off">Off</option><option value="speed">Speed</option><option value="verticalG">Vertical force</option><option value="lateralG">Lateral force</option><option value="longitudinalG">Longitudinal force</option><option value="airtime">Airtime</option></select></label>
    <div data-legend hidden><span data-low></span> <span data-gradient aria-hidden="true" style="display:inline-block;width:100px;height:10px"></span> <span data-high></span></div>
    <div data-proximity-events></div><p data-inspection>Click the coloured track to inspect it.</p>`;
  container.querySelector<HTMLElement>("[data-gradient]")!.style.background = `linear-gradient(to right, ${style.train}, ${style.steel})`;
  const select = container.querySelector<HTMLSelectElement>("[data-metric]")!;
  const legend = container.querySelector<HTMLElement>("[data-legend]")!;
  const inspection = container.querySelector<HTMLElement>("[data-inspection]")!;
  let report: TestingReport | null = null;
  const change = () => {
    const metric = select.value === "off" ? null : select.value as TestingMetric;
    legend.hidden = metric === null || !report?.samples.length;
    if (metric) {
      container.querySelector<HTMLElement>("[data-low]")!.textContent = metrics[metric].low;
      container.querySelector<HTMLElement>("[data-high]")!.textContent = metrics[metric].high;
    }
    onChange(metric && report?.samples.length ? metric : null);
  };
  select.addEventListener("change", change);
  return {
    update(value) {
      report = value;
      container.querySelector<HTMLElement>("[data-notes]")!.textContent = value ? value.notes.join(" ") || "No concerns were noted during this test." : "Run a test to see the results.";
      container.querySelector<HTMLElement>("[data-rating]")!.textContent = value?.rating ? `Excitement: ${number(value.rating.excitement)} · Intensity: ${number(value.rating.intensity)} · Nausea: ${number(value.rating.nausea)}` : "";
      const eventList = container.querySelector<HTMLElement>("[data-proximity-events]")!;
      eventList.replaceChildren();
      if (value?.proximityEvents.length) {
        const heading = document.createElement("h4");
        heading.textContent = `Proximity events (${value.proximityEvents.length})`;
        eventList.append(heading);
        const groups = new Map<string, ProximityEvent[]>();
        for (const event of value.proximityEvents) groups.set(event.kind, [...(groups.get(event.kind) ?? []), event]);
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
        contribution.textContent = `Added ${number(proximityContribution(value.proximityEvents))} excitement from proximity events.`;
        eventList.append(contribution);
      }
      select.disabled = !value?.samples.length;
      inspection.textContent = "Click the coloured track to inspect it.";
      change();
    },
    inspect(index) { if (report?.samples[index]) inspection.textContent = formatSample(report.samples[index]); },
    inspectProximity(index) {
      const event = report?.proximityEvents[index];
      if (event) inspection.textContent = `${formatProximityEvent(event)} · Strength: ${number(event.strength)} · Distance: ${number(event.distance)} m`;
    },
  };
}
