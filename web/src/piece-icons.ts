import type { PieceShape } from "./construction-protocol";

/** Top: seen from above. Side: seen from the left of the track. Twist: an oblique view that shows rolls. */
export type IconView = "top" | "side" | "twist";
type Point = readonly [number, number];

const railHalfWidth = 0.9;
/** Pieces up to this long are drawn to one shared scale, so a large turn reads larger than a small one. */
const referenceLengthMetres = 32;

const angleBetween = (first: readonly number[], second: readonly number[]): number => {
  const dot = first[0] * second[0] + first[1] * second[1] + first[2] * second[2];
  return Math.acos(Math.max(-1, Math.min(1, dot / (Math.hypot(first[0], first[1], first[2]) * Math.hypot(second[0], second[1], second[2])))));
};

/** Whether the piece is still curving where it leaves, so the next piece must continue the curve. */
export function exitKeepsCurving(shape: PieceShape): boolean {
  const segments = shape.points.slice(1).map((point, index) => [0, 1, 2].map((axis) => point[axis] - shape.points[index][axis]));
  const bends = segments.slice(1).map((segment, index) => angleBetween(segments[index], segment));
  const average = bends.reduce((total, bend) => total + bend, 0) / bends.length;
  return average > 0.002 && bends[bends.length - 1] > 0.35 * average;
}

/** The views that show what makes this piece different from a straight. */
export function iconViews(shape: PieceShape): IconView[] {
  const lateral = Math.max(...shape.points.map(([, y]) => Math.abs(y)));
  const heights = shape.points.map(([, , z]) => z);
  const rise = Math.max(...heights) - Math.min(...heights);
  const inverted = shape.points.some(([, , , , , upZ]) => upZ < 0);
  if (inverted && rise < 8) return ["twist"];
  const views: IconView[] = [];
  if (lateral > 0.5) views.push("top");
  if (rise > 0.5) views.push("side");
  return views.length ? views : ["top"];
}

function project(view: IconView, [x, y, z]: readonly number[]): Point {
  switch (view) {
    case "top": return [x, -y];
    case "side": return [x, -z];
    case "twist": return [x + 0.35 * y, -z + 0.55 * y];
  }
}

/** Two rails either side of the centreline, offset along the track's own sideways direction so banks and rolls show. */
function rails(shape: PieceShape): [number[][], number[][]] {
  const left: number[][] = [];
  const right: number[][] = [];
  shape.points.forEach((point, index) => {
    const next = shape.points[Math.min(index + 1, shape.points.length - 1)];
    const previous = shape.points[Math.max(index - 1, 0)];
    const tangent = [next[0] - previous[0], next[1] - previous[1], next[2] - previous[2]];
    const up = [point[3], point[4], point[5]];
    const side = [up[1] * tangent[2] - up[2] * tangent[1], up[2] * tangent[0] - up[0] * tangent[2], up[0] * tangent[1] - up[1] * tangent[0]];
    const length = Math.hypot(...side);
    const offset = side.map((value) => value / length * railHalfWidth);
    left.push([point[0] + offset[0], point[1] + offset[1], point[2] + offset[2]]);
    right.push([point[0] - offset[0], point[1] - offset[1], point[2] - offset[2]]);
  });
  return [left, right];
}

const format = (value: number): string => (Math.round(value * 100) / 100).toString();
const path = (points: readonly Point[]): string => points.map(([x, y], index) => `${index ? "L" : "M"}${format(x)} ${format(y)}`).join("");

/**
 * Ink line drawing of a piece in one view, scaled to fit the box. Small sideways or vertical movements, such as a
 * half-tile offset, are exaggerated so they stay visible at icon size. Colours come from CSS custom properties.
 */
export function pieceIconSvg(shape: PieceShape, view: IconView, width: number, height: number): string {
  const centre = shape.points.map((point) => project(view, point));
  const [leftRail, rightRail] = rails(shape);
  const railPaths = [leftRail, rightRail].map((rail) => rail.map((point) => project(view, point)));
  const all = view === "top" ? railPaths.flat() : centre;
  const minX = Math.min(...all.map(([x]) => x)), maxX = Math.max(...all.map(([x]) => x));
  const minY = Math.min(...all.map(([, y]) => y)), maxY = Math.max(...all.map(([, y]) => y));
  const spanX = Math.max(maxX - minX, 1);
  const padding = 5;
  const fit = Math.min((width - 2 * padding) / spanX, (height - 2 * padding) / Math.max(maxY - minY, 1));
  const shared = (width - 2 * padding) / referenceLengthMetres;
  const scale = Math.max(fit * 0.62, Math.min(fit, shared));
  const minimumSpanY = spanX * 0.3;
  const stretchY = view === "top" && maxY - minY > 0.5 && maxY - minY < minimumSpanY ? Math.min(minimumSpanY / (maxY - minY), (height - 2 * padding) / ((maxY - minY) * scale)) : 1;
  const offsetX = (width - (maxX - minX) * scale) / 2 - minX * scale;
  const offsetY = (height - (maxY - minY) * scale * stretchY) / 2 - minY * scale * stretchY;
  const place = ([x, y]: Point): Point => [x * scale + offsetX, y * scale * stretchY + offsetY];
  const line = centre.map(place);
  const [start, second] = [line[0], line[1]];
  const direction = Math.atan2(second[1] - start[1], second[0] - start[0]);
  const entryBar: Point[] = [[start[0] - Math.sin(direction) * 3.2, start[1] + Math.cos(direction) * 3.2], [start[0] + Math.sin(direction) * 3.2, start[1] - Math.cos(direction) * 3.2]];
  const end = line[line.length - 1];
  const parts = [`<path class="piece-icon__plate" d="${path(line)}"/>`];
  if (view === "side") {
    const base = Math.max(line[0][1], end[1]) + 0.5;
    parts.unshift(`<path class="piece-icon__ground" d="M${padding / 2} ${format(base)}H${width - padding / 2}"/>`);
  }
  if (view === "top" || view === "twist") parts.push(...railPaths.map((rail) => `<path class="piece-icon__rail" d="${path(rail.map(place))}"/>`));
  else parts.push(`<path class="piece-icon__line" d="${path(line)}"/>`);
  if (shape.kind === "Lift") {
    const teeth = line.filter((_, index) => index % 4 === 2).map(([x, y]) => `M${format(x)} ${format(y - 1.6)}v3.2`).join("");
    parts.push(`<path class="piece-icon__detail" d="${teeth}"/>`);
  }
  if (shape.kind === "Brakes") {
    const fins = [10, 16, 22].map((index) => line[index]).map(([x, y]) => `M${format(x - 1.4)} ${format(y - 2.6)}h2.8M${format(x - 1.4)} ${format(y + 2.6)}h2.8`).join("");
    parts.push(`<path class="piece-icon__detail" d="${fins}"/>`);
  }
  if (shape.kind === "Station") {
    parts.unshift(`<rect class="piece-icon__platform" x="${format(line[3][0])}" y="${format(line[3][1] + 2.4)}" width="${format(line[29][0] - line[3][0])}" height="3.4"/>`);
  }
  parts.push(`<path class="piece-icon__entry" d="${path(entryBar)}"/>`);
  if (exitKeepsCurving(shape)) {
    const before = line[line.length - 3];
    const heading = Math.atan2(end[1] - before[1], end[0] - before[0]);
    const wing = (side: number): Point => [end[0] - Math.cos(heading + side * 0.6) * 3.4, end[1] - Math.sin(heading + side * 0.6) * 3.4];
    parts.push(`<path class="piece-icon__continues" d="${path([wing(-1), end, wing(1)])}"/>`);
  } else {
    parts.push(`<circle class="piece-icon__exit" cx="${format(end[0])}" cy="${format(end[1])}" r="1.5"/>`);
  }
  if (shape.entryBank !== "Level" || shape.exitBank !== "Level") parts.push(bankGlyph(shape, width, height));
  return `<svg class="piece-icon piece-icon--${view}" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}" aria-hidden="true" focusable="false">${parts.join("")}</svg>`;
}

const bankAngle: Record<string, number> = { Left: -Math.PI / 4, Level: 0, Right: Math.PI / 4 };

/** The track seen end-on in the corner: a cross tie tilted to the exit bank, with the entry bank faint behind it. */
function bankGlyph(shape: PieceShape, width: number, height: number): string {
  const centre: Point = [width - 7, height - 6];
  const tie = (angle: number): string => {
    const dx = Math.cos(angle) * 4.5, dy = Math.sin(angle) * 4.5;
    return `M${format(centre[0] - dx)} ${format(centre[1] - dy)}L${format(centre[0] + dx)} ${format(centre[1] + dy)}`;
  };
  const entry = shape.entryBank === shape.exitBank ? "" : `<path class="piece-icon__bank-entry" d="${tie(bankAngle[shape.entryBank])}"/>`;
  return `<g class="piece-icon__bank">${entry}<path class="piece-icon__bank-exit" d="${tie(bankAngle[shape.exitBank])}"/><path class="piece-icon__bank-post" d="M${format(centre[0])} ${format(centre[1])}v4"/></g>`;
}
