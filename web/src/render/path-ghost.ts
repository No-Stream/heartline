import { projectWorld, type Camera, type Viewport } from "../camera";
import type { GeometryQuad, Vertex } from "./scene";

export interface PathPreview { x: number; y: number; z: number; slope: number; edges: number; facilityOpenings: number; refusal: string | null }

export function pathRefusal(code: number): string | null {
  const reasons = [
    null,
    "Path route is outside the park",
    "Path route crosses unsupported terrain",
    "Path endpoint heights do not match the route",
    "Path route does not connect across the terrain",
    "Path route overlaps another path",
    "Path route lacks terrain support or clearance",
    "A queue path must have at most one normal path entrance",
    "Path overlaps a facility",
    "Path conflicts with attraction access",
    "Unknown path kind",
    "Path route needs six coordinates",
    "Path coordinates are outside the supported range",
  ];
  if (!Number.isInteger(code) || code < 0 || code >= reasons.length) throw new Error("Invalid path preview refusal");
  return reasons[code];
}

interface DecodedPathPreviewRow { tile: PathPreview | null; refusal: string | null }

const pathPreviewErrorCoordinate = -1;

function decodePathPreviewRows(paths: Int32Array): DecodedPathPreviewRow[] {
  if (paths.length % 9 !== 0) throw new Error("Invalid path preview");
  const preview: DecodedPathPreviewRow[] = [];
  for (let index = 0; index < paths.length; index += 9) {
    if (paths[index + 7] < 0 || paths[index + 7] > 15 || paths[index + 8] < 0 || paths[index + 8] > 15) throw new Error("Invalid path preview edges");
    const [x, y, z, slope, kind, style, refusalCode, edges, facilityOpenings] = paths.slice(index, index + 9);
    const refusal = pathRefusal(refusalCode);
    if (x === pathPreviewErrorCoordinate || y === pathPreviewErrorCoordinate) {
      if (x !== pathPreviewErrorCoordinate || y !== pathPreviewErrorCoordinate || z !== 0 || slope !== 0 || kind !== 0 || style !== 0 || refusal === null || edges !== 0 || facilityOpenings !== 0) {
        throw new Error("Invalid path preview error record");
      }
      preview.push({ tile: null, refusal });
      continue;
    }
    preview.push({ tile: { facilityOpenings, edges, x, y, z, slope, refusal }, refusal });
  }
  return preview;
}

export function decodePathPreview(paths: Int32Array): PathPreview[] {
  return decodePathPreviewRows(paths).flatMap(({ tile }) => tile === null ? [] : [tile]);
}

export function pathPreviewRefusalAt(paths: readonly PathPreview[], x: number, y: number): string | null {
  return paths.find((path) => path.x === x && path.y === y)?.refusal ?? null;
}

export function pathPreviewMessage(paths: Int32Array): string {
  return decodePathPreviewRows(paths).find(({ refusal }) => refusal !== null)?.refusal ?? "";
}

export function pathCornerHeights(z: number, slope: number): number[] {
  const offsets = [[0, 0, 0, 0], [1, 1, 0, 0], [0, 1, 1, 0], [0, 0, 1, 1], [1, 0, 0, 1]][slope];
  if (!offsets) throw new Error("Invalid path slope");
  return offsets.map((offset) => z + offset);
}

export function buildPathGhostQuads(paths: readonly PathPreview[], camera: Camera, viewport: Viewport): GeometryQuad[] {
  return paths.flatMap(({ x, y, z, slope, edges, facilityOpenings, refusal }, index) => {
    const color: GeometryQuad["color"] = refusal === null ? [0.45, 0.86, 0.8, 0.62] : [0.9, 0.3, 0.2, 0.62];
    const heights = pathCornerHeights(z, slope);
    const project = (localX: number, localY: number, lift = 0): Vertex => {
      const height = heights[0] * (1 - localX) * (1 - localY) + heights[1] * localX * (1 - localY) + heights[2] * localX * localY + heights[3] * (1 - localX) * localY;
      const px = x + localX, py = y + localY;
      return { ...projectWorld(camera, viewport, { x: px, y: py, z: height / 4 + lift }), depth: [px + py, px - py, -px - py, -px + py][camera.rotation] + height / 4 + 0.002 + lift, height, u: 0, v: 0 };
    };
    const quad = (points: number[][], quadColor: GeometryQuad["color"], id: string, lift = 0): GeometryQuad => ({
      vertices: points.map(([px, py]) => project(px, py, lift)) as GeometryQuad["vertices"], color: quadColor, kind: "path-preview", id, tileX: x, tileY: y,
    });
    const result = [quad([[0, 0], [1, 0], [1, 1], [0, 1]], color, `path-preview-${index}`)];
    const markers = [
      [[0.35, 0], [0.65, 0], [0.65, 0.16], [0.35, 0.16]],
      [[0.84, 0.35], [1, 0.35], [1, 0.65], [0.84, 0.65]],
      [[0.35, 0.84], [0.65, 0.84], [0.65, 1], [0.35, 1]],
      [[0, 0.35], [0.16, 0.35], [0.16, 0.65], [0, 0.65]],
    ];
    markers.forEach((points, direction) => {
      if ((edges | facilityOpenings) & (1 << direction)) result.push(quad(points, [1, 1, 1, 0.95], `path-preview-${index}-edge-${direction}`, 0.005));
    });
    return result;
  });
}
