import type { ConstructionForceLimitsWire } from "../generated/ConstructionForceLimitsWire";
import type { TestingSample } from "../generated/TestingSample";
import { buildRideGraph, formatGraphReadout, FORCE_AXES, nearestSampleIndex, speedAxisLabel, type ForceAxis, type GraphBox, type RideGraph } from "../ride-graph";
import type { Units } from "../units";

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
const BOX: GraphBox = { width: 440, height: 230, left: 38, right: 40, top: 10, bottom: 26 };
const AXIS_LABELS: Readonly<Record<ForceAxis, string>> = { verticalG: "Vertical", lateralG: "Lateral", longitudinalG: "Forward" };
const EMPTY_READOUT = "Point at the graph, or use the arrow keys when it is focused, to read the forces at that moment of the ride.";

export interface RideGraphView {
  readonly element: HTMLElement;
  update(samples: readonly TestingSample[], limits: ConstructionForceLimitsWire, units: Units): void;
  /** Moves the cursor to a sample from outside the graph, or hides it. */
  setCursor(index: number | undefined): void;
}

function svgElement<Name extends keyof SVGElementTagNameMap>(name: Name, attributes: Record<string, string | number> = {}): SVGElementTagNameMap[Name] {
  const element = document.createElementNS(SVG_NAMESPACE, name);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, String(value));
  return element;
}

/** The g-force and speed graph. `onHover` reports the sample under the pointer, or undefined when it leaves. */
export function createRideGraphView(onHover: (index: number | undefined) => void): RideGraphView {
  const element = document.createElement("figure");
  element.className = "ride-graph";
  element.innerHTML = `
    <figcaption>Forces and speed over the ride</figcaption>
    <svg class="ride-graph__plot" data-testid="ride-graph" viewBox="0 0 ${BOX.width} ${BOX.height}" tabindex="0" role="img" aria-label="Vertical, lateral and forward g-force and speed over ride time"></svg>
    <ul class="ride-graph__legend" data-ride-graph-legend>
      ${FORCE_AXES.map((axis) => `<li data-series="${axis}"><span class="ride-graph__swatch" aria-hidden="true"></span>${AXIS_LABELS[axis]} g</li>`).join("")}
      <li data-series="speed"><span class="ride-graph__swatch" aria-hidden="true"></span><span data-ride-graph-speed-label>Speed</span></li>
    </ul>
    <p class="ride-graph__readout" data-testid="ride-graph-readout" aria-live="polite"></p>`;
  const svg = element.querySelector<SVGSVGElement>("svg")!;
  const readout = element.querySelector<HTMLElement>("[data-testid='ride-graph-readout']")!;
  const speedLabel = element.querySelector<HTMLElement>("[data-ride-graph-speed-label]")!;
  let samples: readonly TestingSample[] = [];
  let graph: RideGraph | undefined;
  let units: Units = "imperial";
  let cursorIndex: number | undefined;

  const cursor = svgElement("g", { class: "ride-graph__cursor", "data-testid": "ride-graph-cursor" });
  cursor.setAttribute("hidden", "");
  const cursorLine = svgElement("line");
  const cursorDots = Object.fromEntries([...FORCE_AXES, "speed"].map((series) => [series, svgElement("circle", { r: 3, "data-series": series })])) as Record<ForceAxis | "speed", SVGCircleElement>;
  cursor.append(cursorLine, ...Object.values(cursorDots));

  function setCursor(index: number | undefined): void {
    cursorIndex = index;
    if (!graph || index === undefined) {
      cursor.setAttribute("hidden", "");
      readout.textContent = EMPTY_READOUT;
      svg.removeAttribute("data-cursor-index");
      return;
    }
    const sample = samples[index];
    const x = graph.timeToX(graph.times[index]);
    cursor.removeAttribute("hidden");
    cursorLine.setAttribute("x1", String(x));
    cursorLine.setAttribute("x2", String(x));
    cursorLine.setAttribute("y1", String(graph.plot.top));
    cursorLine.setAttribute("y2", String(graph.plot.bottom));
    for (const axis of FORCE_AXES) {
      cursorDots[axis].setAttribute("cx", String(x));
      cursorDots[axis].setAttribute("cy", String(graph.gToY(sample[axis])));
    }
    cursorDots.speed.setAttribute("cx", String(x));
    cursorDots.speed.setAttribute("cy", String(graph.speedToY(sample.speed)));
    readout.textContent = formatGraphReadout(sample, graph.times[index], units);
    svg.dataset.cursorIndex = String(index);
  }

  function pointerIndex(event: PointerEvent): number {
    const bounds = svg.getBoundingClientRect();
    return nearestSampleIndex(graph!, (event.clientX - bounds.left) / bounds.width * BOX.width);
  }
  svg.addEventListener("pointermove", (event) => {
    if (!graph) return;
    const index = pointerIndex(event);
    setCursor(index);
    onHover(index);
  });
  svg.addEventListener("pointerleave", () => {
    setCursor(undefined);
    onHover(undefined);
  });
  svg.addEventListener("keydown", (event) => {
    if (!graph || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    event.preventDefault();
    event.stopPropagation();
    const step = (event.key === "ArrowRight" ? 1 : -1) * (event.shiftKey ? 10 : 1);
    const index = Math.max(0, Math.min(samples.length - 1, (cursorIndex ?? 0) + step));
    setCursor(index);
    onHover(index);
  });
  svg.addEventListener("blur", () => {
    setCursor(undefined);
    onHover(undefined);
  });

  const text = (value: string, attributes: Record<string, string | number>): SVGTextElement => {
    const label = svgElement("text", attributes);
    label.textContent = value;
    return label;
  };

  return {
    element,
    update(nextSamples, limits, nextUnits) {
      samples = nextSamples;
      units = nextUnits;
      const built = buildRideGraph(samples, limits, BOX, units);
      graph = built;
      speedLabel.textContent = `Speed (${speedAxisLabel(units)})`;
      const { plot } = graph;
      const children: SVGElement[] = [
        ...built.bands.map((band) => svgElement("rect", { class: "ride-graph__band", "data-series": band.axis, x: plot.left, width: plot.right - plot.left, y: band.top, height: band.bottom - band.top })),
        ...built.forceTicks.map((tick) => svgElement("line", { class: tick.value === 0 ? "ride-graph__zero" : "ride-graph__grid", x1: plot.left, x2: plot.right, y1: tick.position, y2: tick.position })),
        ...built.forceTicks.map((tick) => text(tick.label, { class: "ride-graph__tick", x: plot.left - 4, y: tick.position, "text-anchor": "end", "dominant-baseline": "middle" })),
        ...built.speedTicks.map((tick) => text(tick.label, { class: "ride-graph__tick ride-graph__tick--speed", x: plot.right + 4, y: tick.position, "text-anchor": "start", "dominant-baseline": "middle" })),
        ...built.timeTicks.map((tick) => text(tick.label, { class: "ride-graph__tick", x: tick.position, y: plot.bottom + 14, "text-anchor": "middle" })),
        svgElement("path", { class: "ride-graph__line", "data-series": "speed", d: built.paths.speed }),
        ...FORCE_AXES.map((axis) => svgElement("path", { class: "ride-graph__line", "data-series": axis, d: built.paths[axis] })),
        cursor,
      ];
      svg.replaceChildren(...children);
      svg.dataset.sampleCount = String(samples.length);
      setCursor(cursorIndex !== undefined && cursorIndex < samples.length ? cursorIndex : undefined);
    },
    setCursor,
  };
}
