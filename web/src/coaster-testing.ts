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
  rating: { excitement: number; intensity: number; nausea: number } | null;
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
export function formatSample(sample: TestingSample): string {
  return `Speed: ${number(sample.speed)} m/s · Vertical: ${number(sample.verticalG)} g · Lateral: ${number(sample.lateralG)} g · Longitudinal: ${number(sample.longitudinalG)} g · Airtime: ${sample.airtime ? "yes" : "no"}`;
}

export function createTestingPanel(container: HTMLElement, style: TrackStyle, onChange: (metric: TestingMetric | null) => void): { update(report: TestingReport | null): void; inspect(index: number): void } {
  container.innerHTML = `<h3>Coaster testing</h3><p data-notes></p><p data-rating></p>
    <label>Track overlay <select data-metric><option value="off">Off</option><option value="speed">Speed</option><option value="verticalG">Vertical force</option><option value="lateralG">Lateral force</option><option value="longitudinalG">Longitudinal force</option><option value="airtime">Airtime</option></select></label>
    <div data-legend hidden><span data-low></span> <span data-gradient aria-hidden="true" style="display:inline-block;width:100px;height:10px"></span> <span data-high></span></div>
    <p data-inspection>Click the coloured track to inspect it.</p>`;
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
      select.disabled = !value?.samples.length;
      inspection.textContent = "Click the coloured track to inspect it.";
      change();
    },
    inspect(index) { if (report?.samples[index]) inspection.textContent = formatSample(report.samples[index]); },
  };
}
