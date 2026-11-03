import { projectWorld, type Camera, type Viewport } from "../camera";
import type { GeometryQuad, Vertex } from "./scene";

export interface PathPreview { x: number; y: number; z: number; slope: number; refusal: string | null }

export function pathRefusal(code: number): string | null {
  const reasons = [null, "Path route is outside the park", "Path route crosses unsupported terrain", "Path endpoint heights do not match the route", "Path route does not connect across the terrain", "Path route overlaps another path", "Path route lacks terrain support or clearance", "A queue path must have at most one normal path entrance", "Path overlaps a facility"];
  if (!Number.isInteger(code) || code < 0 || code >= reasons.length) throw new Error("Invalid path preview refusal");
  return reasons[code];
}

export function decodePathPreview(paths: Int32Array): PathPreview[] {
  if (paths.length % 7 !== 0) throw new Error("Invalid path preview");
  const preview: PathPreview[] = [];
  for (let index = 0; index < paths.length; index += 7) {
    preview.push({ x: paths[index], y: paths[index + 1], z: paths[index + 2], slope: paths[index + 3], refusal: pathRefusal(paths[index + 6]) });
  }
  return preview;
}

export function pathPreviewRefusalAt(paths: readonly PathPreview[], x: number, y: number): string | null {
  return paths.find((path) => path.x === x && path.y === y)?.refusal ?? null;
}

export function pathCornerHeights(z: number, slope: number): number[] {
  const offsets = [[0, 0, 0, 0], [1, 1, 0, 0], [0, 1, 1, 0], [0, 0, 1, 1], [1, 0, 0, 1]][slope];
  if (!offsets) throw new Error("Invalid path slope");
  return offsets.map((offset) => z + offset);
}

export function buildPathGhostQuads(paths: readonly PathPreview[], camera: Camera, viewport: Viewport): GeometryQuad[] {
  return paths.map(({ x, y, z, slope, refusal }, index) => {
    const color: GeometryQuad["color"] = refusal === null ? [0.45, 0.86, 0.8, 0.62] : [0.9, 0.3, 0.2, 0.62];
    const heights = pathCornerHeights(z, slope);
    const vertices = [[x, y], [x + 1, y], [x + 1, y + 1], [x, y + 1]].map(([px, py], corner): Vertex => {
      const height = heights[corner];
      return { ...projectWorld(camera, viewport, { x: px, y: py, z: height / 4 }), depth: [px + py, px - py, -px - py, -px + py][camera.rotation] + height / 4 + 0.002, height, u: 0, v: 0 };
    }) as GeometryQuad["vertices"];
    return { vertices, color, kind: "path-preview", id: `path-preview-${index}`, tileX: x, tileY: y };
  });
}
