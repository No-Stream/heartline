// A small painter for buildings and scenery. A model is described once in 3D and drawn from any of
// four sides, so all rotations show the same object: walls, signs and doors stay where they were
// built, and each side only appears when it faces the camera.
import {
  fillPath,
  formatNumber,
  hatchQuad,
  hullOfCircles,
  INK,
  inkLine,
  type Point,
  seededRandom,
  smoothPath,
  subdivide,
  wobble,
} from "./ink.ts";
import { depthOf, project, rotateQuarter, type Vector3 } from "./iso.ts";
import type { Theme } from "./theme.ts";

const LIGHT_DIRECTION: Vector3 = [0.62, 0.05, 0.78];
const CAMERA_DIRECTION: Vector3 = [0.612, 0.612, 0.5];

function dot(first: Vector3, second: Vector3): number {
  return first[0] * second[0] + first[1] * second[1] + first[2] * second[2];
}

function normalOf(points: readonly Vector3[]): Vector3 {
  // Newell's method, so that slightly warped polygons still give a stable normal.
  let [normalX, normalY, normalZ] = [0, 0, 0];
  points.forEach((point, index) => {
    const next = points[(index + 1) % points.length];
    normalX += (point[1] - next[1]) * (point[2] + next[2]);
    normalY += (point[2] - next[2]) * (point[0] + next[0]);
    normalZ += (point[0] - next[0]) * (point[1] + next[1]);
  });
  const length = Math.hypot(normalX, normalY, normalZ) || 1;
  return [normalX / length, normalY / length, normalZ / length];
}

function lerp3(first: Vector3, second: Vector3, fraction: number): Vector3 {
  return [
    first[0] + (second[0] - first[0]) * fraction,
    first[1] + (second[1] - first[1]) * fraction,
    first[2] + (second[2] - first[2]) * fraction,
  ];
}

export type Warp = (point: Vector3) => Vector3;

/**
 * Lean, taper and a gentle bend, applied in model space so that every view shows the same crooked shape.
 * `strength` scales all three, so a theme can straighten its buildings.
 */
export function storybookWarp(lean: readonly [number, number], taper: number, bend: number, height: number, strength: number): Warp {
  return ([x, y, z]) => {
    const heightFraction = Math.max(0, z) / height;
    const narrowing = 1 - taper * strength * heightFraction;
    const curve = Math.sin(heightFraction * Math.PI) * bend * strength;
    return [x * narrowing + lean[0] * strength * heightFraction + curve, y * narrowing + lean[1] * strength * heightFraction - curve * 0.6, z];
  };
}

export interface FaceOptions {
  /** Line weight of the edges; zero leaves the fill without edges. */
  ink?: number;
  /** Apply directional light; decals painted onto a wall usually skip this and use `tone` from the wall. */
  lit?: boolean;
  hatch?: number;
  wobbleAmount?: number;
}

/** A surface that can be drawn on, in its own coordinates: u along it, v up it, both in world units. */
export interface Surface {
  width: number;
  height: number;
  visible: boolean;
  /** Amount of shade to mix into anything painted on this surface. */
  tone: number;
  at(u: number, v: number): Vector3;
}

/**
 * Every method takes design colours and draws them in the model's theme. Line widths are given
 * as in the candy theme and scaled by the theme's line weight.
 */
export class Model {
  readonly view: number;
  readonly theme: Theme;
  private readonly warp: Warp;
  private readonly random: () => number;
  private readonly groups: { depth: number; order: number; markup: string[] }[] = [];
  private current: { depth: number; order: number; markup: string[] };
  private readonly underlay: string[] = [];

  constructor(id: string, view: number, theme: Theme, warp: Warp = (point) => point) {
    this.view = view;
    this.theme = theme;
    this.warp = warp;
    this.random = seededRandom(`${id}/${view}`);
    this.current = { depth: 0, order: 0, markup: [] };
    this.groups.push(this.current);
  }

  /** Model space to view space: warp first, so the crookedness turns with the building. */
  toView(point: Vector3): Vector3 {
    return rotateQuarter(this.warp(point), this.view);
  }

  toScreen(point: Vector3): Point {
    return project(this.toView(point));
  }

  /** Starts a depth-sorted group. All later shapes belong to it. */
  group(center: Vector3, bias = 0): void {
    this.current = { depth: depthOf(this.toView(center)) + bias, order: this.groups.length, markup: [] };
    this.groups.push(this.current);
  }

  raw(markup: string): void {
    this.current.markup.push(markup);
  }

  /** A design colour in this model's theme. */
  color(designColor: string): string {
    return this.theme.color(designColor);
  }

  get ink(): string {
    return this.theme.line.color;
  }

  /** A line width scaled by the theme's line weight. */
  lineWidth(width: number): number {
    return width * this.theme.line.weight;
  }

  private shaded(designColor: string, tone: number): string {
    return this.theme.shade(this.color(designColor), tone);
  }

  private line(points: readonly Point[], width: number, color: string, extra: { opacity?: number; overshoot?: number; taper?: boolean } = {}): string {
    const { line } = this.theme;
    return inkLine(points, this.random, {
      width: width * line.weight,
      color,
      opacity: extra.opacity,
      overshoot: (extra.overshoot ?? 0) * line.weight * line.overshoot,
      taper: extra.taper,
      wobble: line.wobble,
      breaks: line.breaks,
    });
  }

  /** A polygon with a wobbly fill and separately inked edges that slightly overshoot the corners. */
  private polygon(corners: readonly Point[], color: string, width: number, wobbleAmount: number): string {
    const fillOutline = wobble(subdivide(corners, true, 7), wobbleAmount * this.theme.shapeWobble, this.random);
    const fill = fillPath(this.theme, smoothPath(fillOutline, true, 0.45), color);
    if (width <= 0) return fill;
    const edges = corners.map((corner, index) =>
      this.line([corner, corners[(index + 1) % corners.length]], width, this.ink, { overshoot: width * 0.9 }),
    );
    return fill + edges.join("");
  }

  render(): string {
    const sorted = [...this.groups].sort((first, second) => first.depth - second.depth || first.order - second.order);
    return [...this.underlay, ...sorted.flatMap((group) => group.markup)].join("\n");
  }

  shadow(corners: readonly Vector3[], opacity = 0.24): void {
    const screen = corners.map((corner) => this.toScreen(corner));
    const outline = wobble(subdivide(screen, true, 8), 0.7, this.random);
    const { shadow } = this.theme;
    this.underlay.push(`<path d="${smoothPath(outline, true, 0.6)}" fill="${shadow.color}" fill-opacity="${opacity * shadow.opacity}"/>`);
  }

  groundEllipse(radiusX: number, radiusY: number, opacity = 0.24, shiftX = 0, shiftY = 0): void {
    const outline: Vector3[] = Array.from({ length: 20 }, (_, index) => {
      const angle = (index / 20) * Math.PI * 2;
      return [Math.cos(angle) * radiusX + shiftX, Math.sin(angle) * radiusY + shiftY, 0];
    });
    this.shadow(outline, opacity);
  }

  lightAmount(normal: Vector3): number {
    return dot(normal, LIGHT_DIRECTION);
  }

  /** How much to shade a face turned from the light. Top and right faces stay clean colours. */
  private toneFor(normal: Vector3): number {
    return Math.max(0, Math.min(0.34, (0.62 - dot(normal, LIGHT_DIRECTION)) * 0.46));
  }

  face(points: readonly Vector3[], fill: string, { ink = 1.25, lit = true, hatch = 0, wobbleAmount = 0.45 }: FaceOptions = {}): boolean {
    const viewPoints = points.map((point) => this.toView(point));
    const normal = normalOf(viewPoints);
    if (dot(normal, CAMERA_DIRECTION) <= 0.015) return false;
    const color = lit ? this.shaded(fill, this.toneFor(normal)) : this.color(fill);
    const screen = viewPoints.map(project);
    this.raw(this.polygon(screen, color, ink, wobbleAmount));
    const hatchLines = Math.round(hatch * this.theme.hatching);
    if (hatchLines > 0 && screen.length === 4) {
      this.raw(hatchQuad(screen as unknown as [Point, Point, Point, Point], hatchLines, this.random, { width: this.lineWidth(0.7), color: this.ink }));
    }
    return true;
  }

  /** A surface over four corners, wound as for `face`. Its u coordinate runs left to right as seen from outside. */
  surface(corners: readonly [Vector3, Vector3, Vector3, Vector3]): Surface {
    const [bottomLeft, bottomRight, topRight, topLeft] = corners;
    const viewPoints = corners.map((corner) => this.toView(corner));
    const normal = normalOf(viewPoints);
    const width = Math.hypot(bottomRight[0] - bottomLeft[0], bottomRight[1] - bottomLeft[1], bottomRight[2] - bottomLeft[2]);
    const height = Math.hypot(topLeft[0] - bottomLeft[0], topLeft[1] - bottomLeft[1], topLeft[2] - bottomLeft[2]);
    return {
      width,
      height,
      visible: dot(normal, CAMERA_DIRECTION) > 0.015,
      tone: this.toneFor(normal),
      // Faces are wound for the normal; seen from outside on screen, that runs right to left.
      at: (u, v) =>
        lerp3(lerp3(bottomLeft, bottomRight, 1 - u / width), lerp3(topLeft, topRight, 1 - u / width), v / height),
    };
  }

  /** Paints a polygon given in surface coordinates. Colours pick up the surface's shade. */
  paint(surface: Surface, points: readonly (readonly [number, number])[], fill: string, ink = 0.9): void {
    if (!surface.visible) return;
    const screen = points.map(([u, v]) => this.toScreen(surface.at(u, v)));
    this.raw(this.polygon(screen, this.shaded(fill, surface.tone), ink, 0.25));
  }

  rect(surface: Surface, left: number, bottom: number, width: number, height: number, fill: string, ink = 0.9): void {
    this.paint(surface, [[left, bottom], [left + width, bottom], [left + width, bottom + height], [left, bottom + height]], fill, ink);
  }

  /** A door or window shape with a rounded top. */
  arch(surface: Surface, left: number, bottom: number, width: number, height: number, fill: string, ink = 1): void {
    if (!surface.visible) return;
    const radius = width / 2;
    const points: [number, number][] = [[left, bottom], [left + width, bottom]];
    for (let index = 0; index <= 8; index++) {
      const angle = (index / 8) * Math.PI;
      points.push([left + radius + Math.cos(angle) * radius, bottom + height - radius + Math.sin(angle) * radius]);
    }
    const screen = points.map(([u, v]) => this.toScreen(surface.at(u, v)));
    const outline = wobble(subdivide(screen, true, 4), 0.2 * this.theme.shapeWobble, this.random);
    const path = smoothPath(outline, true, 0.6);
    this.raw(fillPath(this.theme, path, this.shaded(fill, surface.tone)));
    if (ink > 0) this.raw(`<path d="${path}" fill="none" stroke="${this.ink}" stroke-width="${this.lineWidth(ink)}" stroke-linejoin="round"/>`);
  }

  ellipseOn(surface: Surface, centerU: number, centerV: number, radiusU: number, radiusV: number, fill: string, ink = 0.9): void {
    if (!surface.visible) return;
    const points: Point[] = Array.from({ length: 16 }, (_, index) => {
      const angle = (index / 16) * Math.PI * 2;
      return this.toScreen(surface.at(centerU + Math.cos(angle) * radiusU, centerV + Math.sin(angle) * radiusV));
    });
    const path = smoothPath(wobble(points, 0.12 * this.theme.shapeWobble, this.random), true);
    this.raw(fillPath(this.theme, path, this.shaded(fill, surface.tone), { strokeWidth: ink > 0 ? this.lineWidth(ink) : 0 }));
  }

  lineOn(surface: Surface, points: readonly (readonly [number, number])[], width = 0.8, color = INK, opacity = 1): void {
    if (!surface.visible) return;
    const screen = points.map(([u, v]) => this.toScreen(surface.at(u, v)));
    this.raw(this.line(screen, width, this.color(color), { opacity }));
  }

  /** A smooth stroke through surface points, with round ends, for curls and hand-lettered shapes. */
  strokeOn(surface: Surface, points: readonly (readonly [number, number])[], width: number, color = INK): void {
    if (!surface.visible) return;
    const screen = points.map(([u, v]) => this.toScreen(surface.at(u, v)));
    this.raw(`<path d="${smoothPath(screen, false)}" fill="none" stroke="${this.color(color)}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round"/>`);
  }

  /** Hand-lettered capitals, drawn in the plane of the surface so that signs sit on their walls. */
  letter(surface: Surface, text: string, centerU: number, bottom: number, letterHeight: number, color = INK, weight = 1.1): void {
    if (!surface.visible) return;
    const letterWidth = letterHeight * 0.68;
    const spacing = letterWidth * 1.28;
    const startU = centerU - ((text.length - 1) * spacing + letterWidth) / 2;
    [...text].forEach((character, index) => {
      if (character === " ") return;
      const strokes = LETTER_STROKES[character];
      if (!strokes) throw new Error(`No hand-lettered shape for "${character}"`);
      const jitter = (this.random() - 0.5) * letterHeight * 0.14;
      for (const stroke of strokes) {
        const screen = stroke.map(([x, y]) =>
          this.toScreen(surface.at(startU + index * spacing + x * letterWidth, bottom + jitter + y * letterHeight)),
        );
        this.raw(`<path d="${smoothPath(screen, false, 0.5)}" fill="none" stroke="${this.color(color)}" stroke-width="${weight}" stroke-linecap="round" stroke-linejoin="round"/>`);
      }
    });
  }

  /** A box whose side surfaces are returned for painting. Front is +x, back is -x, right is +y, left is -y. */
  box(
    centerX: number,
    centerY: number,
    bottom: number,
    sizeX: number,
    sizeY: number,
    height: number,
    color: string,
    { top = color, ink = 1.3, hatch = 0, wobbleAmount = 0.45 }: { top?: string; ink?: number; hatch?: number; wobbleAmount?: number } = {},
  ): BoxSurfaces {
    const [x0, x1] = [centerX - sizeX / 2, centerX + sizeX / 2];
    const [y0, y1] = [centerY - sizeY / 2, centerY + sizeY / 2];
    const [z0, z1] = [bottom, bottom + height];
    const front: [Vector3, Vector3, Vector3, Vector3] = [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]];
    const right: [Vector3, Vector3, Vector3, Vector3] = [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]];
    const back: [Vector3, Vector3, Vector3, Vector3] = [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]];
    const left: [Vector3, Vector3, Vector3, Vector3] = [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]];
    for (const corners of [back, left, right, front]) {
      const surface = this.surface(corners);
      this.face(corners, color, { ink, hatch: surface.tone > 0.12 ? hatch : 0, wobbleAmount });
    }
    this.face([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], top, { ink, wobbleAmount });
    return {
      front: this.surface(front),
      right: this.surface(right),
      back: this.surface(back),
      left: this.surface(left),
    };
  }

  /** Rings of (radius, height) turned around a vertical axis. The convex silhouette is shaded left to right. */
  lathe(
    centerX: number,
    centerY: number,
    profile: readonly (readonly [number, number])[],
    color: string,
    { ink = 1.3, shadeSide = true, topColor }: { ink?: number; shadeSide?: boolean; topColor?: string } = {},
  ): void {
    const rings = profile.map(([radius, height]) =>
      Array.from({ length: 28 }, (_, index) => {
        const angle = (index / 28) * Math.PI * 2;
        return this.toScreen([centerX + Math.cos(angle) * radius, centerY + Math.sin(angle) * radius, height]);
      }),
    );
    const hull = hullOfCircles(rings.flat().map((point) => ({ center: point, radius: 0.01 })));
    const outline = wobble(thinOut(hull, 2.2), 0.22 * this.theme.shapeWobble, this.random);
    const path = smoothPath(outline, true);
    this.raw(fillPath(this.theme, path, this.color(color)));
    if (shadeSide) {
      const clipId = `lathe${this.groups.length}x${Math.round(this.random() * 1e6)}`;
      const xs = hull.map(([x]) => x);
      const [minX, maxX] = [Math.min(...xs), Math.max(...xs)];
      const ys = hull.map(([, y]) => y);
      const [minY, maxY] = [Math.min(...ys), Math.max(...ys)];
      const shadeEdge = minX + (maxX - minX) * 0.36;
      this.raw(
        `<clipPath id="${clipId}"><path d="${path}"/></clipPath>` +
          `<path clip-path="url(#${clipId})" d="M${formatNumber(minX - 2)} ${formatNumber(minY - 2)}H${formatNumber(shadeEdge + 3)}C${formatNumber(shadeEdge - 3)} ${formatNumber(minY + (maxY - minY) * 0.3)} ${formatNumber(shadeEdge - 3)} ${formatNumber(minY + (maxY - minY) * 0.7)} ${formatNumber(shadeEdge + 4)} ${formatNumber(maxY + 2)}H${formatNumber(minX - 2)}Z" fill="${this.theme.shadow.color}" fill-opacity="${this.theme.shadow.roundShadeOpacity}"/>`,
      );
    }
    if (topColor) {
      const top = rings[rings.length - 1];
      this.raw(fillPath(this.theme, smoothPath(top, true), this.color(topColor), { strokeWidth: this.lineWidth(ink * 0.75) }));
    }
    if (ink > 0) this.raw(`<path d="${path}" fill="none" stroke="${this.ink}" stroke-width="${this.lineWidth(ink)}" stroke-linejoin="round"/>`);
  }

  /** A cylinder's side as a surface: u is an arc length around it starting at the +x side, v is height. */
  cylinderSurface(centerX: number, centerY: number, bottom: number, radius: number, height: number, centerAngle: number, arcWidth: number): Surface {
    const angleAt = (u: number): number => centerAngle - (u - arcWidth / 2) / radius;
    const middle = rotateQuarter([Math.cos(centerAngle), Math.sin(centerAngle), 0], this.view);
    const normal: Vector3 = middle;
    return {
      width: arcWidth,
      height,
      visible: dot(normal, CAMERA_DIRECTION) > 0.2,
      tone: this.toneFor(normal),
      at: (u, v) => [centerX + Math.cos(angleAt(u)) * radius, centerY + Math.sin(angleAt(u)) * radius, bottom + v],
    };
  }

  /** A straight round pipe with a wider cap. Separate lathes, because one lathe outline is always convex. */
  pipe(centerX: number, centerY: number, bottom: number, top: number, radius: number, color: string): void {
    this.lathe(centerX, centerY, [[radius, bottom], [radius, top]], color, { ink: 1 });
    this.lathe(centerX, centerY, [[radius + 1.8, top], [radius + 1.8, top + 2], [0.1, top + 4.5]], color, { ink: 1, shadeSide: false });
  }

  /** A cone roof whose sectors alternate colours, like a circus tent, unless the theme turns stripes and scallops off. */
  stripedCone(
    centerX: number,
    centerY: number,
    radius: number,
    bottom: number,
    apex: number,
    colors: readonly string[],
    sectorCount = 12,
  ): void {
    const edge = (index: number): Vector3 => {
      const angle = (index / sectorCount) * Math.PI * 2;
      return [centerX + Math.cos(angle) * radius, centerY + Math.sin(angle) * radius, bottom];
    };
    const apexPoint: Vector3 = [centerX, centerY, apex];
    const baseOutline: Point[] = [];
    for (let index = 0; index < sectorCount; index++) {
      const start = edge(index);
      const end = edge(index + 1);
      const screen = [start, end, apexPoint].map((point) => this.toScreen(point));
      const color = this.theme.props.coneStripes ? colors[index % colors.length] : colors[0];
      const normal = normalOf([start, end, apexPoint].map((point) => this.toView(point)));
      const visible = dot(normal, CAMERA_DIRECTION) > -0.05;
      if (visible) {
        const scallop = this.theme.props.scallops ? this.scallopBetween(screen[0], screen[1]) : [screen[0], screen[1]];
        const outline = [...scallop, screen[2]];
        const seamWidth = this.theme.props.coneStripes ? this.lineWidth(0.9) : 0;
        this.raw(fillPath(this.theme, smoothPath(outline, true, 0.25), this.shaded(color, this.toneFor(normal)), { strokeWidth: seamWidth, lineJoin: true, strokeColor: this.ink }));
        baseOutline.push(...scallop);
      }
    }
  }

  private scallopBetween(start: Point, end: Point): Point[] {
    const [midX, midY] = [(start[0] + end[0]) / 2, (start[1] + end[1]) / 2 + 3.2];
    return [start, [(start[0] + midX) / 2, (start[1] + midY) / 2 + 0.9], [midX, midY], [(end[0] + midX) / 2, (end[1] + midY) / 2 + 0.9], end];
  }

  /** A row of fish-scale shingles on a sloped surface. */
  shingles(surface: Surface, rows: number, columns: number, color: string): void {
    if (!surface.visible) return;
    const rowHeight = surface.height / rows;
    const columnWidth = surface.width / columns;
    for (let row = 0; row < rows; row++) {
      const offset = row % 2 ? columnWidth / 2 : 0;
      for (let column = -1; column <= columns; column++) {
        const left = column * columnWidth + offset;
        const points: [number, number][] = [];
        for (let index = 0; index <= 6; index++) {
          const angle = Math.PI + (index / 6) * Math.PI;
          const pointU = left + columnWidth / 2 + (Math.cos(angle) * columnWidth) / 2;
          const pointV = row * rowHeight + Math.sin(angle) * rowHeight * 0.45 + rowHeight * 0.45;
          if (pointU >= 0 && pointU <= surface.width && pointV >= 0) points.push([pointU, pointV]);
        }
        if (points.length >= 2) this.lineOn(surface, points, 0.6, color, 0.55);
      }
    }
  }

  /** Paints a line between two model-space points, for poles, ropes and brackets. */
  stick(start: Vector3, end: Vector3, width: number, color: string, inkWidth = 1): void {
    const [startScreen, endScreen] = [this.toScreen(start), this.toScreen(end)];
    const segment = `M${formatNumber(startScreen[0])} ${formatNumber(startScreen[1])}L${formatNumber(endScreen[0])} ${formatNumber(endScreen[1])}`;
    if (inkWidth > 0) this.raw(`<path d="${segment}" stroke="${this.ink}" stroke-width="${width + this.lineWidth(inkWidth) * 2}" stroke-linecap="round"/>`);
    this.raw(`<path d="${segment}" stroke="${this.color(color)}" stroke-width="${width}" stroke-linecap="round"/>`);
  }

  /** A smooth tube through model-space points. */
  tube(points: readonly Vector3[], width: number, color: string, inkWidth = 1): void {
    const path = smoothPath(points.map((point) => this.toScreen(point)), false);
    if (inkWidth > 0) this.raw(`<path d="${path}" fill="none" stroke="${this.ink}" stroke-width="${width + this.lineWidth(inkWidth) * 2}" stroke-linecap="round" stroke-linejoin="round"/>`);
    this.raw(`<path d="${path}" fill="none" stroke="${this.color(color)}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round"/>`);
  }

  ball(center: Vector3, radius: number, color: string, { ink = 1.2, highlight = 0.5 }: { ink?: number; highlight?: number } = {}): void {
    const [screenX, screenY] = this.toScreen(center);
    this.raw(this.blob(screenX, screenY, radius, color, ink));
    const highlightOpacity = highlight * this.theme.highlight.opacity;
    if (highlightOpacity > 0) {
      this.raw(`<ellipse cx="${formatNumber(screenX - radius * 0.34)}" cy="${formatNumber(screenY - radius * 0.38)}" rx="${formatNumber(radius * 0.2)}" ry="${formatNumber(radius * 0.28)}" fill="${this.theme.highlight.color}" fill-opacity="${highlightOpacity}"/>`);
    }
  }

  blob(screenX: number, screenY: number, radius: number, color: string, ink = 1.2, wobbleAmount?: number): string {
    const points: Point[] = Array.from({ length: 14 }, (_, index) => {
      const angle = (index / 14) * Math.PI * 2;
      return [screenX + Math.cos(angle) * radius, screenY + Math.sin(angle) * radius];
    });
    const path = smoothPath(wobble(points, (wobbleAmount ?? radius * 0.035) * this.theme.shapeWobble, this.random, radius * 2), true);
    return fillPath(this.theme, path, this.color(color), { strokeWidth: ink > 0 ? this.lineWidth(ink) : 0 });
  }

  /** The curl inside a lollipop tree, in screen space. The colour is a design colour. */
  spiral(screenX: number, screenY: number, radius: number, color: string, turns = 2.6, width = 1.3, direction = 1): void {
    const points: Point[] = [];
    const steps = Math.round(turns * 12);
    for (let index = 0; index <= steps; index++) {
      const fraction = index / steps;
      const angle = direction * fraction * turns * Math.PI * 2;
      points.push([screenX + Math.cos(angle) * radius * fraction, screenY + Math.sin(angle) * radius * fraction * 0.96]);
    }
    this.raw(inkLine(points, this.random, { width, color: this.color(color), taper: true }));
  }

  get randomSource(): () => number {
    return this.random;
  }
}

/** Drops points closer than a minimum distance, so that smoothing does not kink on dense hull samples. */
function thinOut(points: readonly Point[], minimumDistance: number): Point[] {
  const kept: Point[] = [points[0]];
  for (const point of points) {
    const last = kept[kept.length - 1];
    if (Math.hypot(point[0] - last[0], point[1] - last[1]) >= minimumDistance) kept.push(point);
  }
  return kept;
}

export interface BoxSurfaces {
  front: Surface;
  right: Surface;
  back: Surface;
  left: Surface;
}

const LETTER_STROKES: Record<string, readonly (readonly (readonly [number, number])[])[]> = {
  A: [[[0, 0], [0.5, 1], [1, 0]], [[0.2, 0.42], [0.8, 0.42]]],
  B: [[[0, 0], [0, 1], [0.7, 0.98], [0.8, 0.75], [0.6, 0.53], [0, 0.52]], [[0.6, 0.53], [0.95, 0.3], [0.7, 0.02], [0, 0]]],
  C: [[[0.95, 0.85], [0.55, 1], [0.1, 0.8], [0, 0.45], [0.2, 0.1], [0.6, 0], [0.95, 0.15]]],
  D: [[[0, 0], [0, 1], [0.5, 0.95], [0.95, 0.55], [0.6, 0.08], [0, 0]]],
  E: [[[0.9, 1], [0, 1], [0, 0], [0.9, 0]], [[0, 0.5], [0.7, 0.5]]],
  F: [[[0.9, 1], [0, 1], [0, 0]], [[0, 0.5], [0.7, 0.5]]],
  G: [[[0.95, 0.85], [0.55, 1], [0.1, 0.8], [0, 0.45], [0.2, 0.1], [0.6, 0], [0.95, 0.15], [0.95, 0.45], [0.55, 0.45]]],
  H: [[[0, 0], [0, 1]], [[1, 0], [1, 1]], [[0, 0.5], [1, 0.5]]],
  I: [[[0.5, 0], [0.5, 1]]],
  K: [[[0, 0], [0, 1]], [[0.9, 1], [0, 0.45], [1, 0]]],
  L: [[[0, 1], [0, 0], [0.9, 0]]],
  M: [[[0, 0], [0.05, 1], [0.5, 0.35], [0.95, 1], [1, 0]]],
  N: [[[0, 0], [0.05, 1], [0.95, 0], [1, 1]]],
  O: [[[0.5, 1], [0.1, 0.8], [0, 0.5], [0.15, 0.15], [0.5, 0], [0.85, 0.15], [1, 0.5], [0.9, 0.8], [0.5, 1]]],
  P: [[[0, 0], [0, 1], [0.7, 0.98], [0.9, 0.72], [0.6, 0.48], [0, 0.46]]],
  R: [[[0, 0], [0, 1], [0.7, 0.98], [0.9, 0.72], [0.6, 0.48], [0, 0.46]], [[0.45, 0.46], [1, 0]]],
  S: [[[0.95, 0.82], [0.55, 1], [0.1, 0.85], [0.25, 0.55], [0.7, 0.45], [0.95, 0.2], [0.5, 0], [0, 0.15]]],
  T: [[[0, 1], [1, 1]], [[0.5, 1], [0.5, 0]]],
  U: [[[0, 1], [0.02, 0.3], [0.25, 0.05], [0.7, 0.05], [0.97, 0.3], [1, 1]]],
  W: [[[0, 1], [0.22, 0], [0.5, 0.65], [0.78, 0], [1, 1]]],
  Y: [[[0, 1], [0.5, 0.5], [1, 1]], [[0.5, 0.5], [0.5, 0]]],
};
