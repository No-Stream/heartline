// Hand-inked line and shape primitives. Everything returns SVG markup in sprite pixels.

export type Point = readonly [number, number];

export const INK = "#2a1b42";

export function seededRandom(seedText: string): () => number {
  let state = 2166136261;
  for (const character of seedText) {
    state = Math.imul(state ^ character.charCodeAt(0), 16777619);
  }
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

export function formatNumber(value: number): string {
  return String(Math.round(value * 100) / 100);
}

/** Smooth random function of distance along a line, in the range -1..1. */
function smoothNoise(random: () => number, wavelength: number): (distance: number) => number {
  const knots: number[] = [];
  return (distance: number) => {
    const position = distance / wavelength;
    const index = Math.floor(position);
    while (knots.length <= index + 1) knots.push(random() * 2 - 1);
    const blend = (1 - Math.cos((position - index) * Math.PI)) / 2;
    return knots[index] * (1 - blend) + knots[index + 1] * blend;
  };
}

/** Splits a line into short steps so that noise and smoothing have points to work on. */
export function subdivide(points: readonly Point[], closed: boolean, step: number): Point[] {
  const subdivided: Point[] = [];
  const segmentCount = closed ? points.length : points.length - 1;
  for (let segment = 0; segment < segmentCount; segment++) {
    const [startX, startY] = points[segment];
    const [endX, endY] = points[(segment + 1) % points.length];
    const stepCount = Math.max(1, Math.round(Math.hypot(endX - startX, endY - startY) / step));
    for (let stepIndex = 0; stepIndex < stepCount; stepIndex++) {
      const fraction = stepIndex / stepCount;
      subdivided.push([startX + (endX - startX) * fraction, startY + (endY - startY) * fraction]);
    }
  }
  if (!closed) subdivided.push(points[points.length - 1]);
  return subdivided;
}

/** Moves points by a low-frequency offset, which reads as a slightly unsteady hand. */
export function wobble(
  points: readonly Point[],
  amplitude: number,
  random: () => number,
  wavelength = 14,
): Point[] {
  const noiseX = smoothNoise(random, wavelength);
  const noiseY = smoothNoise(random, wavelength);
  let distance = random() * wavelength;
  return points.map(([x, y], index) => {
    if (index > 0) distance += Math.hypot(x - points[index - 1][0], y - points[index - 1][1]);
    return [x + noiseX(distance) * amplitude, y + noiseY(distance) * amplitude];
  });
}

/** Catmull-Rom spline through the points, written as SVG cubic segments. */
export function smoothPath(points: readonly Point[], closed: boolean, tension = 1): string {
  const count = points.length;
  const pointAt = (index: number): Point =>
    closed ? points[(index + count) % count] : points[Math.max(0, Math.min(count - 1, index))];
  const parts = [`M${formatNumber(points[0][0])} ${formatNumber(points[0][1])}`];
  const segmentCount = closed ? count : count - 1;
  for (let index = 0; index < segmentCount; index++) {
    const [previous, start, end, next] = [
      pointAt(index - 1),
      pointAt(index),
      pointAt(index + 1),
      pointAt(index + 2),
    ];
    const controlOneX = start[0] + ((end[0] - previous[0]) / 6) * tension;
    const controlOneY = start[1] + ((end[1] - previous[1]) / 6) * tension;
    const controlTwoX = end[0] - ((next[0] - start[0]) / 6) * tension;
    const controlTwoY = end[1] - ((next[1] - start[1]) / 6) * tension;
    parts.push(
      `C${[controlOneX, controlOneY, controlTwoX, controlTwoY, end[0], end[1]].map(formatNumber).join(" ")}`,
    );
  }
  return parts.join("") + (closed ? "Z" : "");
}

export interface LineOptions {
  width?: number;
  color?: string;
  opacity?: number;
  overshoot?: number;
  taper?: boolean;
}

/**
 * Draws a line as a filled shape whose width changes along its length, like a nib.
 * Tapered ends and a small overshoot at corners are what make edges read as inked.
 */
export function inkLine(
  points: readonly Point[],
  random: () => number,
  {
    width = 1.2,
    color = INK,
    opacity = 1,
    overshoot = 0,
    taper = true,
  }: LineOptions = {},
): string {
  const extended: Point[] = [...points];
  if (overshoot > 0) {
    const extend = (from: Point, to: Point): Point => {
      const length = Math.hypot(to[0] - from[0], to[1] - from[1]) || 1;
      const reach = overshoot * (0.4 + random() * 0.8);
      return [to[0] + ((to[0] - from[0]) / length) * reach, to[1] + ((to[1] - from[1]) / length) * reach];
    };
    extended[0] = extend(points[1], points[0]);
    extended[extended.length - 1] = extend(points[points.length - 2], points[points.length - 1]);
  }
  const center = wobble(subdivide(extended, false, 5), width * 0.28, random, 18);
  const widthNoise = smoothNoise(random, 16);
  const left: Point[] = [];
  const right: Point[] = [];
  let distance = 0;
  center.forEach(([x, y], index) => {
    const [nextX, nextY] = center[Math.min(index + 1, center.length - 1)];
    const [previousX, previousY] = center[Math.max(index - 1, 0)];
    const tangentLength = Math.hypot(nextX - previousX, nextY - previousY) || 1;
    const normalX = -(nextY - previousY) / tangentLength;
    const normalY = (nextX - previousX) / tangentLength;
    if (index > 0) distance += Math.hypot(x - previousX, y - previousY);
    const fraction = index / Math.max(1, center.length - 1);
    const endScale = taper ? 0.45 + 0.55 * Math.min(1, Math.min(fraction, 1 - fraction) * 5) : 1;
    const halfWidth = (width / 2) * (1 + 0.28 * widthNoise(distance)) * endScale;
    left.push([x + normalX * halfWidth, y + normalY * halfWidth]);
    right.push([x - normalX * halfWidth, y - normalY * halfWidth]);
  });
  const outline = [...left, ...right.reverse()];
  const opacityAttribute = opacity < 1 ? ` fill-opacity="${opacity}"` : "";
  return `<path d="${smoothPath(outline, true, 0.6)}" fill="${color}"${opacityAttribute}/>`;
}

/** A polygon with a wobbly fill and separately inked edges that slightly overshoot the corners. */
export function inkPolygon(
  corners: readonly Point[],
  fill: string,
  random: () => number,
  { width = 1.3, wobbleAmount = 0.5, inkEdges = true }: { width?: number; wobbleAmount?: number; inkEdges?: boolean } = {},
): string {
  const fillOutline = wobble(subdivide(corners, true, 7), wobbleAmount, random);
  const fillPath = `<path d="${smoothPath(fillOutline, true, 0.45)}" fill="${fill}"/>`;
  if (!inkEdges) return fillPath;
  const edges = corners.map((corner, index) =>
    inkLine([corner, corners[(index + 1) % corners.length]], random, { width, overshoot: width * 0.9 }),
  );
  return fillPath + edges.join("");
}

/** Shrink-wrapped convex outline around a set of circles. */
export function hullOfCircles(circles: readonly { center: Point; radius: number }[]): Point[] {
  const samples: Point[] = [];
  for (const { center, radius } of circles) {
    for (let index = 0; index < 18; index++) {
      const angle = (index / 18) * Math.PI * 2;
      samples.push([center[0] + Math.cos(angle) * radius, center[1] + Math.sin(angle) * radius]);
    }
  }
  samples.sort((first, second) => first[0] - second[0] || first[1] - second[1]);
  const crossProduct = (origin: Point, first: Point, second: Point): number =>
    (first[0] - origin[0]) * (second[1] - origin[1]) - (first[1] - origin[1]) * (second[0] - origin[0]);
  const halfHull = (ordered: Point[]): Point[] => {
    const hull: Point[] = [];
    for (const point of ordered) {
      while (hull.length >= 2 && crossProduct(hull[hull.length - 2], hull[hull.length - 1], point) <= 0) hull.pop();
      hull.push(point);
    }
    hull.pop();
    return hull;
  };
  return [...halfHull(samples), ...halfHull([...samples].reverse())];
}

/** Hatching lines across a parallelogram-like quad, as in the mockup's shaded faces. */
export function hatchQuad(
  corners: readonly [Point, Point, Point, Point],
  lineCount: number,
  random: () => number,
  { opacity = 0.28, width = 0.7 }: { opacity?: number; width?: number } = {},
): string {
  const lines: string[] = [];
  const interpolate = (first: Point, second: Point, fraction: number): Point => [
    first[0] + (second[0] - first[0]) * fraction,
    first[1] + (second[1] - first[1]) * fraction,
  ];
  for (let index = 1; index <= lineCount; index++) {
    const fraction = (index - 0.5 + (random() - 0.5) * 0.3) / lineCount;
    const start = interpolate(corners[0], corners[1], fraction);
    const end = interpolate(corners[3], corners[2], Math.min(1, fraction + 0.12));
    lines.push(inkLine([start, end], random, { width, opacity }));
  }
  return lines.join("");
}

export function shade(hexColor: string, factor: number): string {
  const value = parseInt(hexColor.slice(1), 16);
  const channel = (shift: number): string =>
    Math.max(0, Math.min(255, Math.round(((value >> shift) & 255) * factor)))
      .toString(16)
      .padStart(2, "0");
  return `#${channel(16)}${channel(8)}${channel(0)}`;
}

/** Shades toward plum instead of black, so shadow sides stay candy-coloured. */
export function shadePlum(hexColor: string, amount: number): string {
  const value = parseInt(hexColor.slice(1), 16);
  const plum = [0x4b, 0x2a, 0x7a];
  const mixed = [16, 8, 0].map((shift, index) =>
    Math.round(((value >> shift) & 255) * (1 - amount) + plum[index] * amount)
      .toString(16)
      .padStart(2, "0"),
  );
  return `#${mixed.join("")}`;
}

export function svgDocument(
  width: number,
  height: number,
  anchorX: number,
  anchorY: number,
  body: string,
  extraAttributes: Record<string, string | number> = {},
): string {
  const extras = Object.entries(extraAttributes)
    .map(([name, value]) => ` ${name}="${value}"`)
    .join("");
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"` +
    ` data-anchor-x="${formatNumber(anchorX)}" data-anchor-y="${formatNumber(anchorY)}"${extras}>\n${body}\n</svg>\n`
  );
}
