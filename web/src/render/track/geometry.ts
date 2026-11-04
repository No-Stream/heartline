import { BRIDGE_LAYOUTS } from "../../bridge-layouts";
import { projectWorld, type Camera, type Viewport } from "../../camera";
import { COASTER_COLORS, RUNNING_RAIL_HEX, type CoasterColorScheme } from "../../coaster-colors";
import type { Atlas, Triangle, Vertex } from "../scene";

export type Vec = [number, number, number];
export interface TrackQuad { vertices: [Vertex, Vertex, Vertex, Vertex]; color: Triangle["color"]; kind: "track" | "support" | "train" | "rider" | "block-boundary" | "block-boundary-fixed"; id: string; tileX: number; tileY: number; page?: number }
export interface TrackStyle { steel: string; wood: string; train: string; ink: string; lineWeight: number }
/** The inks one coaster is drawn in: a paint for each part, and the theme's key line. */
export interface TrackPaint { track: string; supports: string; trains: string; ink: string; lineWeight: number }
/** Support members in tiles: columns are thickest, horizontal ledgers and arms lighter, diagonal braces slimmest. The simulation's support clearance allows up to 0.045. */
const STEEL_MEMBER_RADIUS = { column: 0.04, ledger: 0.028, brace: 0.018 } as const;
const TIMBER_MEMBER_HALF_WIDTH = { column: 0.04, ledger: 0.03, brace: 0.022 } as const;
/** Timber ledgers and braces are drawn without a key line, a shade darker so they still read against the legs. */
const TIMBER_BRACING_SHADE = 0.82;
export const BLOCK_SECTION_PALETTE = ["#38a3a5", "#e07a5f", "#d6a94f", "#7186c1", "#70a288", "#b779a8"] as const;
export const add = (a: Vec, b: Vec): Vec => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
export const scale = (a: Vec, n: number): Vec => [a[0] * n, a[1] * n, a[2] * n];
export const cross = (a: Vec, b: Vec): Vec => [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
export const normalize = (a: Vec): Vec => { const length = Math.hypot(...a); if (length < 1e-8) throw new Error("Degenerate coaster geometry"); return scale(a, 1 / length); };
export const vector = (data: Float32Array, offset: number): Vec => [data[offset], data[offset+1], data[offset+2]];
export const color = (hex: string): Triangle["color"] => [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, 1];
export const shade = (base: Triangle["color"], amount: number): Triangle["color"] => [base[0] * amount, base[1] * amount, base[2] * amount, base[3]];
function geometryId(encoded: number): { id: number; steel: boolean; woodenTrack: boolean } {
  if (!Number.isInteger(encoded)) throw new Error("Invalid track geometry ID");
  if (encoded < 0) return { id: -encoded - 1, steel: true, woodenTrack: false };
  if (encoded >= 1_000_000) return { id: encoded - 1_000_000, steel: false, woodenTrack: true };
  return { id: encoded, steel: false, woodenTrack: false };
}
export function trackStyle(atlas: Atlas): TrackStyle {
  const { themeStyle } = atlas;
  if (!themeStyle || !themeStyle.steel || !themeStyle.wood || !themeStyle.train || !themeStyle.ink) throw new Error("Atlas is missing coaster theme style");
  if (!Number.isFinite(themeStyle.lineWeight) || themeStyle.lineWeight <= 0) throw new Error("Invalid coaster line weight");
  return themeStyle;
}

export function trackPaint(style: TrackStyle, colors: CoasterColorScheme): TrackPaint {
  return { track: COASTER_COLORS[colors.track].hex, supports: COASTER_COLORS[colors.supports].hex, trains: COASTER_COLORS[colors.trains].hex, ink: style.ink, lineWeight: style.lineWeight };
}

function sectionColorAt(distanceMetres: number, boundariesMetres: readonly number[]): Triangle["color"] {
  let sectionIndex = 0;
  for (const boundaryMetres of boundariesMetres) {
    if (boundaryMetres > 0 && boundaryMetres <= distanceMetres) sectionIndex++;
  }
  const hex = BLOCK_SECTION_PALETTE[sectionIndex % BLOCK_SECTION_PALETTE.length];
  return color(hex);
}

function sectionColorsForRails(rails: Float32Array, boundariesMetres: readonly number[], railDistancesMetres: Float32Array): Triangle["color"][] {
  if (rails.length % 38 !== 0) throw new Error("Block-section coloring requires paired coaster rails");
  if (railDistancesMetres.length !== rails.length / 19) throw new Error("Block-section distances do not match coaster rails");
  for (let index = 0; index < boundariesMetres.length; index++) {
    if (!Number.isFinite(boundariesMetres[index]) || boundariesMetres[index] < 0 || (index > 0 && boundariesMetres[index] < boundariesMetres[index - 1])) {
      throw new RangeError("Invalid block-section boundary order");
    }
  }
  const railColors: Triangle["color"][] = [];
  let previousEndMetres = 0;
  for (let offset = 0; offset < rails.length; offset += 38) {
    const distanceOffset = offset / BRIDGE_LAYOUTS.RAIL.length;
    const startMetres = railDistancesMetres[distanceOffset];
    const endMetres = railDistancesMetres[distanceOffset + 1];
    if (rails[offset] !== rails[offset + 19] || !Number.isFinite(startMetres) || !Number.isFinite(endMetres) || startMetres !== previousEndMetres || endMetres <= startMetres) {
      throw new RangeError("Invalid coaster rail distance span");
    }
    const fillColor = sectionColorAt((startMetres + endMetres) / 2, boundariesMetres);
    railColors.push(fillColor, fillColor);
    previousEndMetres = endMetres;
  }
  if (boundariesMetres.some((boundaryMetres) => boundaryMetres > previousEndMetres)) {
    throw new RangeError("Block boundary is outside the rendered coaster rails");
  }
  return railColors;
}

export function interpolateTrainPoses(previous: Float32Array, current: Float32Array, interpolation: number): Float32Array {
  if (current.length % 11 || previous.length % 11 || interpolation < 0 || interpolation > 1) throw new Error("Invalid train snapshot");
  const result = new Float32Array(current);
  for (let i = 0; i < current.length; i += BRIDGE_LAYOUTS.TRAIN_POSE.length) {
    if (previous[i] !== current[i]) continue;
    for (let j = 1; j < 10; j++) result[i+j] = previous[i+j] + (current[i+j] - previous[i+j]) * interpolation;
    const side = normalize(vector(result, i+4));
    const rawUp = vector(result, i+7);
    const dot = side[0]*rawUp[0] + side[1]*rawUp[1] + side[2]*rawUp[2];
    result.set(side, i+4);
    result.set(normalize(add(rawUp, scale(side, -dot))), i+7);
  }
  return result;
}

/**
 * Projects world-space boxes and tubes into inked, depth-tagged quads, collected in `quads`. Closed boxes and tubes skip
 * the faces that point away from the viewer, which the depth test would hide anyway.
 */
export function createQuadBuilder(camera: Camera, viewport: Viewport, ink: string, lineWeight: number) {
  const quads: TrackQuad[] = [];
  const towardViewer: Vec = ([[1,1,1], [1,-1,1], [-1,-1,1], [-1,1,1]] as const)[camera.rotation] as Vec;
  const boxFaces = [[0,1,2,3], [4,7,6,5], [0,4,5,1], [1,5,6,2], [2,6,7,3], [3,7,4,0]];
  const towardViewerOf = (point: Vec) => point[0]*towardViewer[0] + point[1]*towardViewer[1] + point[2]*towardViewer[2];
  /** The faces of a closed box whose outward side faces the viewer. */
  const visibleFaces = (corners: Vec[]) => {
    let centreTowardViewer = 0;
    for (const corner of corners) centreTowardViewer += towardViewerOf(corner);
    centreTowardViewer /= corners.length;
    return boxFaces.filter((face) => {
      let faceTowardViewer = 0;
      for (const index of face) faceTowardViewer += towardViewerOf(corners[index]);
      return faceTowardViewer / face.length > centreTowardViewer;
    });
  };
  const inkColor = color(ink);
  const depthAxis = ([[1,1], [1,-1], [-1,-1], [-1,1]] as const)[camera.rotation];
  const project = (point: Vec): Vertex => {
    const screen = projectWorld(camera, viewport, { x: point[0], y: point[1], z: point[2] });
    return { x: screen.x, y: screen.y, depth: point[0]*depthAxis[0] + point[1]*depthAxis[1] + point[2], height: point[2]*4, u: 0, v: 0 };
  };
  /** The projection is affine, so a weighted blend of projected corners projects the same blend of world points. */
  const blend = (vertices: readonly Vertex[], weights: readonly number[]): Vertex => {
    const result: Vertex = { x: 0, y: 0, depth: 0, height: 0, u: 0, v: 0 };
    for (let index = 0; index < 4; index++) {
      result.x += vertices[index].x * weights[index];
      result.y += vertices[index].y * weights[index];
      result.depth += vertices[index].depth * weights[index];
      result.height += vertices[index].height * weights[index];
    }
    return result;
  };
  const push = (vertices: Vertex[], faceColor: TrackQuad["color"], kind: TrackQuad["kind"], id: string, tile: Vec) => quads.push({ vertices: vertices as TrackQuad["vertices"], color: faceColor, kind, id, tileX: Math.floor(tile[0]), tileY: Math.floor(tile[1]) });
  const edgeLength = (first: Vec, second: Vec) => Math.hypot(first[0] - second[0], first[1] - second[1], first[2] - second[2]);
  /** Corners 0-3 ring one end and 4-7 the other, in the same winding. Each face is a fill inset by a key-line border. */
  const box = (corners: Vec[], kind: TrackQuad["kind"], id: string, fillColor: TrackQuad["color"], openEnds = false) => {
    const projected = corners.map(project);
    const inkWidth = (openEnds ? 0.012 : 0.008) * lineWeight;
    for (const face of openEnds ? boxFaces.slice(2) : visibleFaces(corners)) {
      const faceVertices = face.map((index) => projected[index]);
      const horizontalInset = openEnds ? 0 : Math.min(0.18, inkWidth / edgeLength(corners[face[0]], corners[face[1]]));
      const verticalInset = Math.min(openEnds ? 0.25 : 0.18, inkWidth / edgeLength(corners[face[0]], corners[face[3]]));
      const inner = faceVertices.map((_, index) => {
        const nextInset = index % 2 === 0 ? horizontalInset : verticalInset;
        const previousInset = index % 2 === 0 ? verticalInset : horizontalInset;
        const around = [faceVertices[index], faceVertices[(index+1)%4], faceVertices[(index+3)%4], faceVertices[(index+2)%4]];
        return blend(around, [(1-nextInset)*(1-previousInset), nextInset*(1-previousInset), previousInset*(1-nextInset), nextInset*previousInset]);
      });
      push(inner, fillColor, kind, id, corners[0]);
      for (const edge of openEnds ? [0, 2] : [0, 1, 2, 3]) {
        const next = (edge+1)%4;
        push([faceVertices[edge], faceVertices[next], inner[next], inner[edge]], inkColor, kind, id, corners[0]);
      }
    }
  };
  /** A box drawn as six flat faces with no key line, for small parts painted in the key line itself. */
  const solid = (corners: Vec[], kind: TrackQuad["kind"], id: string, fillColor: TrackQuad["color"]) => {
    const projected = corners.map(project);
    for (const face of visibleFaces(corners)) push(face.map((index) => projected[index]), fillColor, kind, id, corners[0]);
  };
  const square = (center: Vec, side: Vec, up: Vec, width: number): Vec[] => [[-1,-1], [1,-1], [1,1], [-1,1]].map(([s,u]) => add(add(center, scale(side, s*width)), scale(up, u*width)));
  const beam = (start: Vec, end: Vec, sideStart: Vec, sideEnd: Vec, upStart: Vec, upEnd: Vec, width: number, kind: TrackQuad["kind"], id: string, fillColor: TrackQuad["color"], openEnds = false) => {
    box([...square(start, sideStart, upStart, width), ...square(end, sideEnd, upEnd, width)], kind, id, fillColor, openEnds);
  };
  /** A slim beam drawn without a key line, cheaper than an inked one where many thin members repeat. */
  const solidBeam = (start: Vec, end: Vec, side: Vec, up: Vec, width: number, kind: TrackQuad["kind"], id: string, fillColor: TrackQuad["color"]) =>
    solid([...square(start, side, up, width), ...square(end, side, up, width)], kind, id, fillColor);
  const tube = (start: Vec, end: Vec, sideStart: Vec, sideEnd: Vec, upStart: Vec, upEnd: Vec, radius: number, kind: TrackQuad["kind"], id: string, baseColor: TrackQuad["color"]) => {
    const sides = 8;
    const ring = (center: Vec, side: Vec, up: Vec): Vec[] => Array.from({ length: sides }, (_, index) => {
      const angle = index * Math.PI * 2 / sides;
      return add(add(center, scale(side, Math.cos(angle) * radius)), scale(up, Math.sin(angle) * radius));
    });
    const startRing = ring(start, sideStart, upStart);
    const endRing = ring(end, sideEnd, upEnd);
    const axisTowardViewer = (towardViewerOf(start) + towardViewerOf(end)) / 2;
    const startVertices = startRing.map(project);
    const endVertices = endRing.map(project);
    for (let index = 0; index < sides; index++) {
      const next = (index + 1) % sides;
      const faceTowardViewer = (towardViewerOf(startRing[index]) + towardViewerOf(startRing[next]) + towardViewerOf(endRing[next]) + towardViewerOf(endRing[index])) / 4;
      if (faceTowardViewer <= axisTowardViewer) continue;
      const angle = (index + 0.5) * Math.PI * 2 / sides;
      const brightness = 0.76 + 0.24 * (Math.cos(angle) + 1) / 2;
      push([startVertices[index], startVertices[next], endVertices[next], endVertices[index]], shade(baseColor, brightness), kind, id, start);
    }
  };
  return { quads, box, solid, beam, solidBeam, tube };
}

export function buildTrackQuads(rails: Float32Array, ties: Float32Array, supports: Float32Array, camera: Camera, viewport: Viewport, style: TrackPaint, sectionBoundariesMetres?: readonly number[], railDistancesMetres?: Float32Array): TrackQuad[] {
  if (rails.length % 19 || ties.length % 13 || supports.length % 7) throw new Error("Invalid coaster snapshot");
  if (sectionBoundariesMetres?.length && railDistancesMetres === undefined) throw new Error("Block-section coloring requires exact rail distances");
  const sectionColors = sectionBoundariesMetres?.length ? sectionColorsForRails(rails, sectionBoundariesMetres, railDistancesMetres!) : undefined;
  const { quads, beam, solidBeam, tube } = createQuadBuilder(camera, viewport, style.ink, style.lineWeight);
  const track = color(style.track); const supportColor = color(style.supports); const runningRail = color(RUNNING_RAIL_HEX);
  for (let i = 0; i < rails.length; i += BRIDGE_LAYOUTS.RAIL.length) {
    const id = geometryId(rails[i]);
    const start = vector(rails, i+1), end = vector(rails, i+10);
    const sideStart = normalize(vector(rails, i+4)), sideEnd = normalize(vector(rails, i+13));
    const upStart = normalize(vector(rails, i+7)), upEnd = normalize(vector(rails, i+16));
    const sectionColor = sectionColors?.[i / BRIDGE_LAYOUTS.RAIL.length];
    if (id.woodenTrack) {
      beam(start, end, sideStart, sideEnd, upStart, upEnd, 0.055, "track", `track-${id.id}`, sectionColor ?? track, true);
      beam(add(start, scale(upStart, 0.065)), add(end, scale(upEnd, 0.065)), sideStart, sideEnd, upStart, upEnd, 0.01, "track", `track-${id.id}`, runningRail, true);
    }
    else if (id.steel) tube(start, end, sideStart, sideEnd, upStart, upEnd, 0.035, "track", `track-${id.id}`, sectionColor ?? track);
    else beam(start, end, sideStart, sideEnd, upStart, upEnd, 0.035, "track", `track-${id.id}`, sectionColor ?? track, true);
  }
  for (let i = 0; i < ties.length; i += BRIDGE_LAYOUTS.TIE.length) {
    const id = geometryId(ties[i]);
    const up = normalize(vector(ties, i+7)), tangent = normalize(vector(ties, i+10));
    const start = vector(ties, i+1), end = vector(ties, i+4);
    if (id.steel) tube(start, end, up, up, tangent, tangent, 0.025, "track", `track-${id.id}`, track);
    else beam(start, end, up, up, tangent, tangent, 0.025, "track", `track-${id.id}`, track);
  }
  for (let i = 0; i < supports.length; i += BRIDGE_LAYOUTS.SUPPORT.length) {
    const id = geometryId(supports[i]);
    const start = vector(supports, i+1), end = vector(supports, i+4);
    const direction = normalize(add(end, scale(start, -1)));
    const side = normalize(cross(direction, Math.abs(direction[2]) > 0.9 ? [1,0,0] : [0,0,1]));
    const up = normalize(cross(direction, side));
    const member = Math.abs(direction[2]) > 0.999 ? "column" : Math.abs(direction[2]) < 0.001 ? "ledger" : "brace";
    if (id.steel) tube(start, end, side, side, up, up, STEEL_MEMBER_RADIUS[member], "support", `support-${id.id}`, supportColor);
    else if (member === "column") beam(start, end, side, side, up, up, TIMBER_MEMBER_HALF_WIDTH[member], "support", `support-${id.id}`, supportColor);
    else solidBeam(start, end, side, up, TIMBER_MEMBER_HALF_WIDTH[member], "support", `support-${id.id}`, shade(supportColor, TIMBER_BRACING_SHADE));
    if (start[0] === end[0] && start[1] === end[1] && end[2] > start[2]) {
      beam(start, add(start, [0, 0, 0.05]), [1, 0, 0], [1, 0, 0], [0, 1, 0], [0, 1, 0], 0.1, "support", `footer-${id.id}`, supportColor);
    }
  }
  return quads;
}
