import { projectWorld, type Camera, type Viewport } from "../../camera";
import type { ScreenPoint } from "../../input/actions";
import type { Triangle, Vertex } from "../scene";
import { BLOCK_SECTION_PALETTE, type TrackQuad, type TrackStyle } from "./geometry";

export interface BlockSectionBoundary {
  readonly boundaryMetres: number;
  readonly editable: boolean;
  readonly editableIndex: number | null;
  readonly trackKind: string;
}

type Vector3 = [number, number, number];

interface RailSegment {
  readonly start: Vector3;
  readonly end: Vector3;
  readonly side: Vector3;
  readonly up: Vector3;
  readonly startMetres: number;
  readonly lengthMetres: number;
}

const add = (first: Vector3, second: Vector3): Vector3 => [first[0] + second[0], first[1] + second[1], first[2] + second[2]];
const scale = (vector: Vector3, amount: number): Vector3 => [vector[0] * amount, vector[1] * amount, vector[2] * amount];
const distance = (first: Vector3, second: Vector3): number => Math.hypot(first[0] - second[0], first[1] - second[1], first[2] - second[2]);
const vector = (data: Float32Array, offset: number): Vector3 => [data[offset], data[offset + 1], data[offset + 2]];

function normalize(vector: Vector3): Vector3 {
  const length = Math.hypot(...vector);
  if (length < 1e-8) throw new Error("Degenerate block-section frame");
  return scale(vector, 1 / length);
}

function projectedVertex(point: Vector3, camera: Camera, viewport: Viewport): Vertex {
  const screen = projectWorld(camera, viewport, { x: point[0], y: point[1], z: point[2] });
  const depthByRotation = [point[0] + point[1], point[0] - point[1], -point[0] - point[1], -point[0] + point[1]][camera.rotation];
  return { ...screen, depth: depthByRotation + point[2], height: point[2] * 4, u: 0, v: 0 };
}

function railSegments(rails: Float32Array, railDistancesMetres: Float32Array): RailSegment[] {
  if (rails.length % 38 !== 0) throw new Error("Invalid paired coaster rail snapshot");
  if (railDistancesMetres.length !== rails.length / 19) throw new Error("Rail distances do not match paired coaster rails");
  if (rails.length === 0) return [];

  const segments: RailSegment[] = [];
  let previousEndMetres = 0;
  for (let offset = 0; offset < rails.length; offset += 38) {
    if (rails[offset] !== rails[offset + 19]) throw new Error("Mismatched paired coaster rail IDs");
    const start = scale(add(vector(rails, offset + 1), vector(rails, offset + 20)), 0.5);
    const end = scale(add(vector(rails, offset + 10), vector(rails, offset + 29)), 0.5);
    const startSide = scale(add(vector(rails, offset + 4), vector(rails, offset + 23)), 0.5);
    const endSide = scale(add(vector(rails, offset + 13), vector(rails, offset + 32)), 0.5);
    const startUp = scale(add(vector(rails, offset + 7), vector(rails, offset + 26)), 0.5);
    const endUp = scale(add(vector(rails, offset + 16), vector(rails, offset + 35)), 0.5);
    const distanceOffset = offset / 19;
    const startMetres = railDistancesMetres[distanceOffset];
    const endMetres = railDistancesMetres[distanceOffset + 1];
    if (!Number.isFinite(startMetres) || !Number.isFinite(endMetres) || startMetres !== previousEndMetres || endMetres <= startMetres || distance(start, end) < 1e-8) {
      throw new Error("Invalid coaster rail distance span");
    }
    segments.push({
      start,
      end,
      side: normalize(add(startSide, endSide)),
      up: normalize(add(startUp, endUp)),
      startMetres,
      lengthMetres: endMetres - startMetres,
    });
    previousEndMetres = endMetres;
  }
  return segments;
}

function validateBoundaries(sections: readonly BlockSectionBoundary[]): void {
  const editableIndices = new Set<number>();
  let previousDistance = -1;
  for (const section of sections) {
    if (!Number.isFinite(section.boundaryMetres) || section.boundaryMetres < 0 || section.boundaryMetres <= previousDistance) {
      throw new RangeError("Invalid block-section boundary order");
    }
    previousDistance = section.boundaryMetres;
    if (typeof section.trackKind !== "string") throw new TypeError("Invalid block-section track kind");
    if (section.editable) {
      if (!Number.isInteger(section.editableIndex) || section.editableIndex! < 0 || editableIndices.has(section.editableIndex!)) {
        throw new RangeError("Invalid editable block-section index");
      }
      editableIndices.add(section.editableIndex!);
    } else if (section.editableIndex !== null) {
      throw new RangeError("Fixed block sections cannot have an editable index");
    }
  }
}

function pointOnTrack(segments: readonly RailSegment[], targetMetres: number): { center: Vector3; side: Vector3; up: Vector3 } {
  const totalMetres = segments.at(-1)!.startMetres + segments.at(-1)!.lengthMetres;
  if (targetMetres > totalMetres + 1e-4) throw new RangeError("Block boundary is outside the rendered coaster rails");
  const target = Math.min(targetMetres, totalMetres);
  const segment = segments.find((candidate) => target <= candidate.startMetres + candidate.lengthMetres) ?? segments.at(-1)!;
  const fraction = Math.max(0, Math.min(1, (target - segment.startMetres) / segment.lengthMetres));
  return {
    center: add(segment.start, scale(add(segment.end, scale(segment.start, -1)), fraction)),
    side: segment.side,
    up: segment.up,
  };
}

function blockColor(sectionIndex: number): Triangle["color"] {
  const hex = BLOCK_SECTION_PALETTE[sectionIndex % BLOCK_SECTION_PALETTE.length];
  return [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, 1];
}

function colorFromHex(hex: string): Triangle["color"] {
  return [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, 1];
}

export function buildBlockBoundaryQuads(
  rails: Float32Array,
  railDistancesMetres: Float32Array,
  sections: readonly BlockSectionBoundary[],
  camera: Camera,
  viewport: Viewport,
  style: TrackStyle,
): TrackQuad[] {
  validateBoundaries(sections);
  const segments = railSegments(rails, railDistancesMetres);
  if (segments.length === 0 || sections.length === 0) return [];
  const totalMetres = segments.at(-1)!.startMetres + segments.at(-1)!.lengthMetres;
  const markers: TrackQuad[] = [];
  for (let index = 0; index < sections.length; index++) {
    const section = sections[index];
    if (section.boundaryMetres > totalMetres + 1e-4) throw new RangeError("Block boundary is outside the rendered coaster rails");
    const { center, side, up } = pointOnTrack(segments, section.boundaryMetres);
    const markerCenter = add(center, scale(up, 0.09));
    const editable = section.editable;
    const editableIndex = section.editableIndex;
    const markerKind = editable ? "block-boundary" : "block-boundary-fixed";
    const markerId = editable ? `block-boundary-${editableIndex}` : `block-boundary-fixed-${index}`;
    const appendMarker = (halfWidth: number, halfHeight: number, fillColor: Triangle["color"]): void => {
      const corners = [
        add(add(markerCenter, scale(side, -halfWidth)), scale(up, -halfHeight)),
        add(add(markerCenter, scale(side, halfWidth)), scale(up, -halfHeight)),
        add(add(markerCenter, scale(side, halfWidth)), scale(up, halfHeight)),
        add(add(markerCenter, scale(side, -halfWidth)), scale(up, halfHeight)),
      ];
      markers.push({
        vertices: corners.map((corner) => projectedVertex(corner, camera, viewport)) as TrackQuad["vertices"],
        color: fillColor,
        kind: markerKind,
        id: markerId,
        tileX: Math.floor(center[0]),
        tileY: Math.floor(center[1]),
      });
    };
    appendMarker(0.24, 0.095, colorFromHex(style.ink));
    appendMarker(0.22, 0.075, blockColor(index));
  }
  return markers;
}

export function nearestBlockBoundaryMetres(
  rails: Float32Array,
  railDistancesMetres: Float32Array,
  screenPoint: ScreenPoint,
  camera: Camera,
  viewport: Viewport,
): number | undefined {
  const segments = railSegments(rails, railDistancesMetres);
  if (segments.length === 0) return undefined;
  if (!Number.isFinite(screenPoint.x) || !Number.isFinite(screenPoint.y)) throw new RangeError("Invalid block-section drag position");
  let nearestDistanceSquared = Infinity;
  let nearestTrackMetres: number | undefined;
  for (const segment of segments) {
    const projectedStart = projectWorld(camera, viewport, { x: segment.start[0], y: segment.start[1], z: segment.start[2] });
    const projectedEnd = projectWorld(camera, viewport, { x: segment.end[0], y: segment.end[1], z: segment.end[2] });
    const deltaX = projectedEnd.x - projectedStart.x;
    const deltaY = projectedEnd.y - projectedStart.y;
    const screenLengthSquared = deltaX * deltaX + deltaY * deltaY;
    const fraction = screenLengthSquared === 0 ? 0 : Math.max(0, Math.min(1, ((screenPoint.x - projectedStart.x) * deltaX + (screenPoint.y - projectedStart.y) * deltaY) / screenLengthSquared));
    const nearestX = projectedStart.x + deltaX * fraction;
    const nearestY = projectedStart.y + deltaY * fraction;
    const distanceSquared = (screenPoint.x - nearestX) ** 2 + (screenPoint.y - nearestY) ** 2;
    if (distanceSquared < nearestDistanceSquared) {
      nearestDistanceSquared = distanceSquared;
      nearestTrackMetres = segment.startMetres + segment.lengthMetres * fraction;
    }
  }
  return nearestTrackMetres;
}

