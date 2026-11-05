import { projectVertex } from "./projection";
import { unprojectGround, type Camera, type Viewport } from "../camera";
import { pathCornerHeights } from "./path-ghost";
import type { CutSide, ScreenBounds } from "./cutaway";

export interface AtlasFrame {
  width: number; height: number; anchor: { x: number; y: number };
  properties?: Record<string, string>;
  placements: Record<string, { page: number; x: number; y: number; width: number; height: number }>;
}
export interface Atlas {
  themeStyle?: { steel: string; wood: string; train: string; ink: string; lineWeight: number };
  pages: Record<string, { file: string; width: number; height: number }[]>;
  frames: Record<string, AtlasFrame>;
}
export interface WorldSprite { id: string; sprite: string; x: number; y: number; height: number; view?: string; animation?: string; frame?: number; scale?: number; heightDepth?: boolean }
export interface WorldView { dimensions: Uint16Array; surfaces: Int32Array; paths: Int32Array; pathMetadata?: Int32Array; facilityOpenings?: Int32Array; carved?: Int32Array; sprites?: readonly WorldSprite[]; undergroundLevel?: number }
export type Pick = { kind: "terrain" | "water" | "path" | "cliff" | "carved-floor" | "cavern-wall"; x: number; y: number; height: number } | { kind: string; id: string; x: number; y: number; height: number };
export interface Vertex { x: number; y: number; depth: number; height: number; u: number; v: number }
/** `page` is an atlas page, or one of `SHAPE_PAGES` for an untextured fill. `light` is how much the surface keeps its own colour at night (see `gradeColor`). */
export interface Triangle { vertices: [Vertex, Vertex, Vertex]; page: number; color: readonly [number, number, number, number]; kind: string; tileX: number; tileY: number; height: number; id?: string; light?: number; cut?: CutSide }
export interface Scene { triangles: Triangle[] }
export interface GeometryQuad { vertices: [Vertex, Vertex, Vertex, Vertex]; page?: number; color: Triangle["color"]; kind: string; id: string; tileX: number; tileY: number; light?: number; lightSourceHeight?: number }
/** Untextured fills. Round shapes are drawn inside the unit square given by the vertices' u and v. */
export const SHAPE_PAGES = { flat: -1, glow: -2, disc: -3, softDisc: -4, lightPool: -5 } as const;
/** Drawn but never picked: lights and the dashed path preview. */
export const UNPICKABLE_KINDS: ReadonlySet<string> = new Set(["path-preview", "light", "block-boundary-fixed", "tunnel-portal"]);
interface HeightPoint { x: number; y: number; height: number }
const WHITE = [1, 1, 1, 1] as const;
const QUAD_TRIANGLES = [[0, 1, 2], [0, 2, 3]] as const;
export const UNDERGROUND_CONTENT_DEPTH_BIAS = 4;

function interpolateVertex(first: Vertex, second: Vertex, fraction: number): Vertex {
  return {
    x: first.x + (second.x - first.x) * fraction,
    y: first.y + (second.y - first.y) * fraction,
    depth: first.depth + (second.depth - first.depth) * fraction,
    height: first.height + (second.height - first.height) * fraction,
    u: first.u + (second.u - first.u) * fraction,
    v: first.v + (second.v - first.v) * fraction,
  };
}

function clipTriangleAtHeight(vertices: readonly [Vertex, Vertex, Vertex], level: number, keepBelow: boolean): Vertex[] {
  const clipped: Vertex[] = [];
  for (let index = 0; index < vertices.length; index++) {
    const current = vertices[index];
    const next = vertices[(index + 1) % vertices.length];
    const currentInside = keepBelow ? current.height <= level : current.height >= level;
    const nextInside = keepBelow ? next.height <= level : next.height >= level;
    if (currentInside) clipped.push(current);
    if (currentInside !== nextInside) {
      const fraction = (level - current.height) / (next.height - current.height);
      clipped.push(interpolateVertex(current, next, fraction));
    }
  }
  return clipped;
}

function clipHeightPolygon(vertices: readonly HeightPoint[], level: number, keepBelow: boolean): HeightPoint[] {
  const clipped: HeightPoint[] = [];
  for (let index = 0; index < vertices.length; index++) {
    const current = vertices[index];
    const next = vertices[(index + 1) % vertices.length];
    const currentInside = keepBelow ? current.height <= level : current.height >= level;
    const nextInside = keepBelow ? next.height <= level : next.height >= level;
    if (currentInside) clipped.push(current);
    if (currentInside !== nextInside) {
      const fraction = (level - current.height) / (next.height - current.height);
      clipped.push({
        x: current.x + (next.x - current.x) * fraction,
        y: current.y + (next.y - current.y) * fraction,
        height: level,
      });
    }
  }
  return clipped;
}

function appendClippedTriangle(quads: GeometryQuad[], vertices: readonly [Vertex, Vertex, Vertex], source: GeometryQuad, level: number): void {
  const polygon = clipTriangleAtHeight(vertices, level, true);
  for (let index = 1; index < polygon.length - 1; index++) {
    const triangle = [polygon[0], polygon[index], polygon[index + 1]] as const;
    quads.push({ ...source, vertices: [triangle[0], triangle[1], triangle[2], triangle[2]] });
  }
}

function appendPolygonTriangles(triangles: Triangle[], polygon: readonly Vertex[], metadata: Omit<Triangle, "vertices" | "height">): void {
  for (let index = 1; index < polygon.length - 1; index += 2) {
    const first = [polygon[0], polygon[index], polygon[index + 1]] as [Vertex, Vertex, Vertex];
    const second = index + 2 < polygon.length
      ? [polygon[0], polygon[index + 1], polygon[index + 2]] as [Vertex, Vertex, Vertex]
      : [polygon[0], polygon[index + 1], polygon[index + 1]] as [Vertex, Vertex, Vertex];
    for (const vertices of [first, second]) {
      triangles.push({ ...metadata, vertices, height: Math.max(...vertices.map((vertex) => vertex.height)) });
    }
  }
}

/** Keeps geometry at or below the selected underground cut level. */
export function clipGeometryAboveLevel(quads: readonly GeometryQuad[], level: number, depthBias = 0): GeometryQuad[] {
  if (!Number.isInteger(level)) throw new RangeError("Underground level must be an integer height step");
  const clipped: GeometryQuad[] = [];
  for (const quad of quads) {
    if (quad.vertices.every((vertex) => vertex.height <= level)) {
      clipped.push(depthBias === 0 ? quad : { ...quad, vertices: quad.vertices.map((vertex) => ({ ...vertex, depth: vertex.depth + depthBias })) as GeometryQuad["vertices"] });
      continue;
    }
    if (quad.vertices.every((vertex) => vertex.height > level)) continue;
    const [first, second] = QUAD_TRIANGLES.map((indices) => indices.map((index) => quad.vertices[index]) as [Vertex, Vertex, Vertex]);
    appendClippedTriangle(clipped, first, quad, level);
    appendClippedTriangle(clipped, second, quad, level);
  }
  return depthBias === 0 ? clipped : clipped.map((quad) => ({
    ...quad,
    vertices: quad.vertices.map((vertex) => ({ ...vertex, depth: vertex.depth + depthBias })) as GeometryQuad["vertices"],
  }));
}

export function geometryIntersectsViewport(quad: GeometryQuad, viewport: Viewport): boolean {
  return !(quad.vertices.every((vertex) => vertex.x < 0) || quad.vertices.every((vertex) => vertex.x > viewport.width) || quad.vertices.every((vertex) => vertex.y < 0) || quad.vertices.every((vertex) => vertex.y > viewport.height));
}

export function appendGeometry(scene: Scene, quads: readonly GeometryQuad[], viewport?: Viewport): void {
  for (const quad of quads) {
    if (viewport && !geometryIntersectsViewport(quad, viewport)) continue;
    for (const indices of QUAD_TRIANGLES) scene.triangles.push({
      vertices: indices.map((index) => quad.vertices[index]) as Triangle["vertices"],
      page: quad.page ?? SHAPE_PAGES.flat, color: quad.color, kind: quad.kind, id: quad.id, light: quad.light,
      tileX: quad.tileX, tileY: quad.tileY, height: Math.max(...quad.vertices.map((vertex) => vertex.height)),
    });
  }
}

export function terrainHeightAt(world: WorldView, x: number, y: number): number {
  const [width, height] = world.dimensions;
  if (!Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0 || x >= width || y >= height) {
    throw new RangeError("Terrain position is outside the world");
  }
  const tileX = Math.floor(x);
  const tileY = Math.floor(y);
  const surfaceIndex = (tileY * width + tileX) * 6;
  const northWest = world.surfaces[surfaceIndex];
  const northEast = world.surfaces[surfaceIndex + 1];
  const southEast = world.surfaces[surfaceIndex + 2];
  const southWest = world.surfaces[surfaceIndex + 3];
  const fractionX = x - tileX;
  const fractionY = y - tileY;

  if (fractionX >= fractionY) {
    return northWest * (1 - fractionX) + northEast * (fractionX - fractionY) + southEast * fractionY;
  }
  return northWest * (1 - fractionY) + southEast * fractionX + southWest * (fractionY - fractionX);
}

function frameFor(atlas: Atlas, sprite: string, view = "any", animation = "static", frame = 0, scale = "1"): AtlasFrame {
  const key = `${sprite}/${view}/${animation}/${frame}`;
  const result = atlas.frames[key];
  if (!result || !result.placements[scale]) throw new Error(`Missing atlas frame ${key}`);
  return result;
}

/** `bounds`, in canvas pixels, limits which tiles are built; it defaults to the viewport. */
export function buildScene(world: WorldView, camera: Camera, viewport: Viewport, atlas: Atlas, scale = "1", waterFrame = 0, bounds: ScreenBounds = { left: 0, top: 0, right: viewport.width, bottom: viewport.height }): Scene {
  const [width, height] = world.dimensions;
  if (!width || !height || world.dimensions.length !== 2 || world.surfaces.length !== width * height * 6 || world.paths.length % 4 !== 0) throw new Error("Invalid world snapshot");
  const undergroundLevel = world.undergroundLevel;
  if (undergroundLevel !== undefined && !Number.isInteger(undergroundLevel)) throw new RangeError("Underground level must be an integer height step");
  const triangles: Triangle[] = [];
  const grass = frameFor(atlas, "ground/grass_light_0", "any", "static", 0, scale);
  if (world.pathMetadata && world.pathMetadata.length !== world.paths.length / 4 * 3) throw new Error("Invalid path metadata");
  const paths = new Map<number, [number, number, number, number, number][]>();
  const facilityOpenings = new Map<string, number>();
  if (world.facilityOpenings && world.facilityOpenings.length % 4 !== 0) throw new Error("Invalid facility openings");
  for (let index = 0; index < (world.facilityOpenings?.length ?? 0); index += 4) {
    const [x, y, z, edges] = world.facilityOpenings!.subarray(index, index + 4);
    if (x < 0 || y < 0 || x >= width || y >= height || edges < 1 || edges > 15) throw new Error("Invalid facility opening");
    const key = `${x},${y},${z}`;
    facilityOpenings.set(key, (facilityOpenings.get(key) ?? 0) | edges);
  }
  const carved = new Map<number, [number, number][]>();
  if (world.carved && world.carved.length % 4 !== 0) throw new Error("Invalid carved spans");
  for (let i = 0; i < (world.carved?.length ?? 0); i += 4) {
    const [x, y, bottom, top] = world.carved!.subarray(i, i + 4);
    if (x < 0 || y < 0 || x >= width || y >= height || bottom >= top) throw new Error("Invalid carved span");
    const key = y * width + x;
    carved.set(key, [...(carved.get(key) ?? []), [bottom, top]]);
  }
  for (let index = 0; index < world.paths.length; index += 4) {
    const [x, y, z, slope] = world.paths.subarray(index, index + 4);
    if (x < 0 || y < 0 || x >= width || y >= height || slope < 0 || slope > 4) throw new Error("Invalid path");
    const [kind, style, edges] = world.pathMetadata?.subarray(index / 4 * 3, index / 4 * 3 + 3) ?? [0, 0, 0];
    if (kind < 0 || kind > 1 || style < 0 || style > 255 || edges < 0 || edges > 15) throw new Error("Invalid path metadata");
    const key = y * width + x;
    paths.set(key, [...(paths.get(key) ?? []), [z, slope, kind, style, edges]]);
  }
  const pathCorners = pathCornerHeights;
  const point = (x: number, y: number, heightSteps: number): Vertex => {
    return { ...projectVertex(camera, viewport, { x, y, z: heightSteps / 4 }), height: heightSteps };
  };
  const addQuad = (corners: [Vertex, Vertex, Vertex, Vertex], kind: Triangle["kind"], x: number, y: number, color: Triangle["color"], frame?: AtlasFrame, id?: string, textureCoordinates?: number[][], heightLimit?: number, depthBias = 0): void => {
    const vertices = corners.map((corner) => ({ ...corner })) as [Vertex, Vertex, Vertex, Vertex];
    for (const vertex of vertices) vertex.depth += depthBias;
    let page = -1;
    if (frame) {
      const placement = frame.placements[scale];
      const size = atlas.pages[scale]?.[placement.page];
      if (!size) throw new Error("Missing atlas page");
      page = placement.page;
      const uv = textureCoordinates ?? [[0.5, 0.1], [0.944444, 0.5], [0.5, 0.9], [0.055556, 0.5]];
      for (let i = 0; i < 4; i++) {
        const coordinate = uv[textureCoordinates ? i : (i + camera.rotation) % 4];
        vertices[i].u = (placement.x + coordinate[0] * placement.width) / size.width;
        vertices[i].v = (placement.y + coordinate[1] * placement.height) / size.height;
      }
    }
    for (const indices of QUAD_TRIANGLES) {
      const source = [vertices[indices[0]], vertices[indices[1]], vertices[indices[2]]] as [Vertex, Vertex, Vertex];
      if (heightLimit === undefined) triangles.push({ vertices: source, page, color, kind, tileX: x, tileY: y, height: Math.max(...source.map((vertex) => vertex.height)), id });
      else appendPolygonTriangles(triangles, clipTriangleAtHeight(source, heightLimit, true), { page, color, kind, tileX: x, tileY: y, id });
    }
  };
  const addTunnelPortal = (start: [number, number], end: [number, number], bottom: number, top: number, x: number, y: number): void => {
    const portalHeight = top - bottom;
    if (portalHeight <= 0) return;
    const positionAlongEdge = (fraction: number): [number, number] => [
      start[0] + (end[0] - start[0]) * fraction,
      start[1] + (end[1] - start[1]) * fraction,
    ];
    const outerStart = positionAlongEdge(0.12);
    const innerStart = positionAlongEdge(0.21);
    const innerEnd = positionAlongEdge(0.79);
    const outerEnd = positionAlongEdge(0.88);
    const topBeamBottom = top - Math.min(1, portalHeight * 0.18);
    const earth = [0.48, 0.37, 0.28, 1] as const;
    const lintel = [0.58, 0.46, 0.35, 1] as const;
    const portalPoint = (position: [number, number], height: number): Vertex => point(position[0], position[1], height);
    addQuad([
      portalPoint(outerStart, bottom), portalPoint(innerStart, bottom),
      portalPoint(innerStart, top), portalPoint(outerStart, top),
    ], "tunnel-portal", x, y, earth, undefined, undefined, undefined, undergroundLevel);
    addQuad([
      portalPoint(innerEnd, bottom), portalPoint(outerEnd, bottom),
      portalPoint(outerEnd, top), portalPoint(innerEnd, top),
    ], "tunnel-portal", x, y, earth, undefined, undefined, undefined, undergroundLevel);
    addQuad([
      portalPoint(innerStart, topBeamBottom), portalPoint(innerEnd, topBeamBottom),
      portalPoint(innerEnd, top), portalPoint(innerStart, top),
    ], "tunnel-portal", x, y, lintel, undefined, undefined, undefined, undergroundLevel);
  };
  const addFlatPolygon = (polygon: readonly HeightPoint[], level: number, kind: Triangle["kind"], x: number, y: number, color: Triangle["color"], frame?: AtlasFrame, depthBias = 0): void => {
    if (polygon.length < 3) return;
    const vertices = polygon.map(({ x: worldX, y: worldY }) => {
      const vertex = point(worldX, worldY, level);
      vertex.depth += depthBias;
      return vertex;
    });
    let page = -1;
    if (frame) {
      const placement = frame.placements[scale];
      const size = atlas.pages[scale]?.[placement.page];
      if (!size) throw new Error("Missing atlas page");
      page = placement.page;
      const defaultCoordinates = [[0.5, 0.1], [0.944444, 0.5], [0.5, 0.9], [0.055556, 0.5]];
      const coordinates = defaultCoordinates.map((_, index) => defaultCoordinates[(index + camera.rotation) % 4]);
      for (let index = 0; index < vertices.length; index++) {
        const fractionX = polygon[index].x - x;
        const fractionY = polygon[index].y - y;
        const topU = coordinates[0][0] * (1 - fractionX) + coordinates[1][0] * fractionX;
        const topV = coordinates[0][1] * (1 - fractionX) + coordinates[1][1] * fractionX;
        const bottomU = coordinates[3][0] * (1 - fractionX) + coordinates[2][0] * fractionX;
        const bottomV = coordinates[3][1] * (1 - fractionX) + coordinates[2][1] * fractionX;
        const coordinateU = topU * (1 - fractionY) + bottomU * fractionY;
        const coordinateV = topV * (1 - fractionY) + bottomV * fractionY;
        vertices[index].u = (placement.x + coordinateU * placement.width) / size.width;
        vertices[index].v = (placement.y + coordinateV * placement.height) / size.height;
      }
    }
    appendPolygonTriangles(triangles, vertices, { page, color, kind, tileX: x, tileY: y });
  };
  let minHeight = 0; let maxHeight = 0;
  for (let i = 0; i < world.surfaces.length; i += 6) {
    const northWest = world.surfaces[i]; const northEast = world.surfaces[i + 1];
    const southEast = world.surfaces[i + 2]; const southWest = world.surfaces[i + 3];
    minHeight = Math.min(minHeight, northWest, northEast, southEast, southWest);
    maxHeight = Math.max(maxHeight, northWest, northEast, southEast, southWest, world.surfaces[i + 4] ? world.surfaces[i + 5] : maxHeight);
  }
  for (let i = 2; i < world.paths.length; i += 4) { minHeight = Math.min(minHeight, world.paths[i]); maxHeight = Math.max(maxHeight, world.paths[i] + 1); }
  for (let i = 2; i < (world.carved?.length ?? 0); i += 4) minHeight = Math.min(minHeight, world.carved![i]);
  const groundCorners = [minHeight, maxHeight].flatMap((heightSteps) => [
    { x: bounds.left, y: bounds.top }, { x: bounds.right, y: bounds.top }, { x: bounds.right, y: bounds.bottom }, { x: bounds.left, y: bounds.bottom },
  ].map((screen) => unprojectGround(camera, viewport, screen, heightSteps / 4)));
  const startX = Math.max(0, Math.floor(Math.min(...groundCorners.map((point) => point.x))) - 2);
  const endX = Math.min(width, Math.ceil(Math.max(...groundCorners.map((point) => point.x))) + 2);
  const startY = Math.max(0, Math.floor(Math.min(...groundCorners.map((point) => point.y))) - 2);
  const endY = Math.min(height, Math.ceil(Math.max(...groundCorners.map((point) => point.y))) + 2);
  for (let y = startY; y < endY; y++) for (let x = startX; x < endX; x++) {
    const index = (y * width + x) * 6;
    const corners = [...world.surfaces.subarray(index, index + 4)];
    const positions: [number, number][] = [[x,y], [x+1,y], [x+1,y+1], [x,y+1]];
    const quad = (heights: readonly number[]): [Vertex, Vertex, Vertex, Vertex] => positions.map(([px, py], i) => point(px, py, heights[i])) as [Vertex, Vertex, Vertex, Vertex];
    const terrain = quad(corners);
    const tileHeights = [...corners];
    for (const [bottom] of carved.get(y * width + x) ?? []) tileHeights.push(bottom);
    if (world.surfaces[index + 4]) tileHeights.push(world.surfaces[index + 5]);
    for (const [z, slope] of paths.get(y * width + x) ?? []) tileHeights.push(...pathCorners(z, slope));
    for (const [dx, dy] of [[0,-1], [1,0], [0,1], [-1,0]]) {
      const nx = x + dx; const ny = y + dy;
      if (nx >= 0 && ny >= 0 && nx < width && ny < height) {
        const neighborIndex = (ny * width + nx) * 6;
        tileHeights.push(...world.surfaces.subarray(neighborIndex, neighborIndex + 4));
        for (const [bottom] of carved.get(ny * width + nx) ?? []) tileHeights.push(bottom);
      } else tileHeights.push(0);
    }
    const projectedBounds = [...quad(positions.map(() => Math.min(...tileHeights))), ...quad(positions.map(() => Math.max(...tileHeights)))];
    const minX = Math.min(...projectedBounds.map((v) => v.x)); const maxX = Math.max(...projectedBounds.map((v) => v.x));
    const minY = Math.min(...projectedBounds.map((v) => v.y)); const maxY = Math.max(...projectedBounds.map((v) => v.y));
    if (maxX < bounds.left - 100 || minX > bounds.right + 100 || maxY < bounds.top - 100 || minY > bounds.bottom + 100) continue;
    const tileCarvings = carved.get(y * width + x) ?? [];
    const opening = tileCarvings.find(([, top]) => corners.every((corner) => top >= corner));
    if (!opening) addQuad(terrain, "terrain", x, y, WHITE, grass, undefined, undefined, undergroundLevel);
    if (undergroundLevel !== undefined && corners.some((corner) => corner > undergroundLevel) &&
        !tileCarvings.some(([bottom, top]) => bottom <= undergroundLevel && undergroundLevel < top)) {
      const surfacePoints = positions.map(([px, py], corner) => ({ x: px, y: py, height: corners[corner] }));
      for (const indices of QUAD_TRIANGLES) {
        const polygon = clipHeightPolygon(indices.map((corner) => surfacePoints[corner]), undergroundLevel, false);
        addFlatPolygon(polygon, undergroundLevel, "terrain", x, y, [0.46, 0.34, 0.24, 1]);
      }
    }
    const exposedCorners = opening ? corners.map((corner) => Math.min(corner, opening[0])) : corners;
    for (let edge = 0; edge < 4; edge++) {
      const next = (edge + 1) % 4;
      const dx = [0,1,0,-1][edge]; const dy = [-1,0,1,0][edge];
      const nx = x + dx; const ny = y + dy;
      const neighborIndex = (ny * width + nx) * 6;
      const adjacent = nx >= 0 && ny >= 0 && nx < width && ny < height ? [world.surfaces[neighborIndex + [3,0,1,2][edge]], world.surfaces[neighborIndex + [2,3,0,1][edge]]] : [0,0];
      const mapEdge = nx < 0 || ny < 0 || nx >= width || ny >= height;
      let neighborOpening: [number, number] | undefined;
      if (!mapEdge) {
        const neighborCorners = world.surfaces.subarray(neighborIndex, neighborIndex + 4);
        neighborOpening = (carved.get(ny * width + nx) ?? []).find(([, top]) => neighborCorners.every((corner) => top >= corner));
        if (neighborOpening) {
          adjacent[0] = Math.min(adjacent[0], neighborOpening[0]);
          adjacent[1] = Math.min(adjacent[1], neighborOpening[0]);
        }
      }
      if (opening && !mapEdge && !neighborOpening) {
        const connectedSpan = (carved.get(ny * width + nx) ?? []).find(([neighborBottom, neighborTop]) =>
          neighborTop > opening[0] && neighborBottom < opening[1]);
        if (connectedSpan) {
          const portalBottom = Math.max(opening[0], connectedSpan[0]);
          const portalTop = Math.min(Math.min(...corners), connectedSpan[1]);
          addTunnelPortal(positions[edge], positions[next], portalBottom, portalTop, x, y);
        }
      }
      if (!mapEdge && exposedCorners[edge] <= adjacent[0] && exposedCorners[next] <= adjacent[1]) continue;
      const midpoint = point((positions[edge][0] + positions[next][0]) / 2, (positions[edge][1] + positions[next][1]) / 2, 0);
      const center = point(x + 0.5, y + 0.5, 0);
      if (midpoint.y <= center.y) continue;
      const side = midpoint.x < center.x ? "left" : "right";
      const variant = ((x * 31 + y * 17) % 3 + 3) % 3;
      const face = frameFor(atlas, `ground/cliff_${mapEdge ? "" : "step_"}${side}_${variant}`, "any", "static", 0, scale);
      const faceHeight = mapEdge ? 5.5 : 1;
      const leftTop = side === "left" ? [face.anchor.x, face.anchor.y] : [face.anchor.x - 32, face.anchor.y + 16];
      const rightTop = side === "left" ? [face.anchor.x + 32, face.anchor.y + 16] : [face.anchor.x, face.anchor.y];
      const leftFirst = terrain[edge].x < terrain[next].x;
      const firstUV = leftFirst ? leftTop : rightTop;
      const nextUV = leftFirst ? rightTop : leftTop;
      const steps = mapEdge ? 1 : Math.ceil(Math.max(exposedCorners[edge] - adjacent[0], exposedCorners[next] - adjacent[1]));
      for (let step = 0; step < steps; step++) {
        const floorFirst = mapEdge ? -Infinity : Math.min(exposedCorners[edge], adjacent[0]);
        const floorNext = mapEdge ? -Infinity : Math.min(exposedCorners[next], adjacent[1]);
        const topFirst = Math.max(exposedCorners[edge] - step * faceHeight, floorFirst);
        const topNext = Math.max(exposedCorners[next] - step * faceHeight, floorNext);
        const bottomFirst = Math.max(topFirst - faceHeight, floorFirst);
        const bottomNext = Math.max(topNext - faceHeight, floorNext);
        const uv = [firstUV, nextUV, [nextUV[0], nextUV[1] + (topNext - bottomNext) * 8], [firstUV[0], firstUV[1] + (topFirst - bottomFirst) * 8]].map(([u, v]) => [u / face.width, v / face.height]);
        addQuad([point(...positions[edge], topFirst), point(...positions[next], topNext), point(...positions[next], bottomNext), point(...positions[edge], bottomFirst)], "cliff", x, y, WHITE, face, undefined, uv, undergroundLevel);
      }
    }
    if (undergroundLevel !== undefined) {
      for (const [bottom, top] of tileCarvings) {
        if (bottom <= undergroundLevel) {
          addFlatPolygon(positions.map(([px, py]) => ({ x: px, y: py, height: bottom })), bottom, "carved-floor", x, y, [0.54, 0.42, 0.31, 1], undefined, UNDERGROUND_CONTENT_DEPTH_BIAS);
        }
        if (top >= Math.max(...corners)) continue;
        for (let edge = 0; edge < 4; edge++) {
          const next = (edge + 1) % 4;
          const dx = [0,1,0,-1][edge]; const dy = [-1,0,1,0][edge];
          const nx = x + dx; const ny = y + dy;
          if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
          const neighborSpans = carved.get(ny * width + nx) ?? [];
          let exposed: [number, number][] = [[bottom, top]];
          for (const [neighborBottom, neighborTop] of neighborSpans) {
            exposed = exposed.flatMap(([first, second]) => {
              if (neighborTop <= first || neighborBottom >= second) return [[first, second]];
              return [
                ...(neighborBottom > first ? [[first, neighborBottom] as [number, number]] : []),
                ...(neighborTop < second ? [[neighborTop, second] as [number, number]] : []),
              ];
            });
          }
          for (const [wallBottom, wallTop] of exposed) {
            const visibleTop = Math.min(wallTop, undergroundLevel);
            if (wallBottom >= visibleTop) continue;
            const wall = [
              point(...positions[edge], visibleTop), point(...positions[next], visibleTop),
              point(...positions[next], wallBottom), point(...positions[edge], wallBottom),
            ] as [Vertex, Vertex, Vertex, Vertex];
            addQuad(wall, "cavern-wall", x, y, [0.32, 0.25, 0.19, 1]);
          }
        }
      }
    }
    if (world.surfaces[index + 4]) {
      const waterLevel = world.surfaces[index + 5];
      if ((undergroundLevel === undefined || waterLevel <= undergroundLevel) && corners.some((corner) => corner < waterLevel)) {
        const waterFrameAtlas = frameFor(atlas, "ground/water", "any", "loop", waterFrame, scale);
        if (corners.every((corner) => corner <= waterLevel)) {
          addQuad(quad([waterLevel, waterLevel, waterLevel, waterLevel]), "water", x, y, WHITE, waterFrameAtlas);
        } else {
          const surfacePoints = positions.map(([px, py], corner) => ({ x: px, y: py, height: corners[corner] }));
          for (const indices of QUAD_TRIANGLES) {
            const polygon = clipHeightPolygon(indices.map((corner) => surfacePoints[corner]), waterLevel, true);
            addFlatPolygon(polygon, waterLevel, "water", x, y, WHITE, waterFrameAtlas);
          }
        }
      }
    }
    for (const path of paths.get(y * width + x) ?? []) {
      const [z, slope, kind, style, edges] = path;
      const heights = pathCorners(z, slope);
      const undergroundPathFits = tileCarvings.some(([bottom, top]) => bottom <= Math.min(...heights) && top >= Math.max(...heights));
      if (undergroundLevel === undefined) {
        if (heights.some((height, corner) => height < corners[corner]) &&
            (!opening || heights.some((height) => height < opening[0] || height >= opening[1]))) continue;
      } else if (heights.some((height, corner) => height < corners[corner]) && !undergroundPathFits &&
                 (!opening || heights.some((height) => height < opening[0] || height >= opening[1]))) continue;
      const pathQuad = quad(heights);
      for (const vertex of pathQuad) vertex.depth += 0.001 + (undergroundLevel === undefined ? 0 : UNDERGROUND_CONTENT_DEPTH_BIAS);
      const directions = [
        { dx: 1, dy: 0, edge: [1, 2], neighborEdge: [0, 3] },
        { dx: 0, dy: 1, edge: [3, 2], neighborEdge: [0, 1] },
        { dx: -1, dy: 0, edge: [0, 3], neighborEdge: [1, 2] },
        { dx: 0, dy: -1, edge: [0, 1], neighborEdge: [3, 2] },
      ];
      const directionBits = [2, 4, 8, 1];
      const connections = [0, 1, 2, 3].map((direction) =>
        edges & directionBits[(direction - camera.rotation + 4) % 4] ? "1" : "0",
      ).join("");
      const pathColors: Triangle["color"][] = [WHITE, [0.82, 0.9, 1, 1], [1, 0.85, 0.7, 1], [0.8, 1, 0.8, 1]];
      addQuad(pathQuad, "path", x, y, pathColors[style % pathColors.length], frameFor(atlas, `ground/path_${connections}`, "any", "static", 0, scale), undefined, undefined, undergroundLevel, undergroundLevel === undefined ? 0 : UNDERGROUND_CONTENT_DEPTH_BIAS);
      if (kind === 1) {
        const ink = atlas.themeStyle?.ink ?? "#333333";
        const color: Triangle["color"] = [parseInt(ink.slice(1, 3), 16) / 255, parseInt(ink.slice(3, 5), 16) / 255, parseInt(ink.slice(5, 7), 16) / 255, 1];
        for (let direction = 0; direction < 4; direction++) {
          const { edge } = directions[direction];
          const connected = ((edges | (facilityOpenings.get(`${x},${y},${z}`) ?? 0)) & directionBits[direction]) !== 0;
          if (connected) continue;
          const [first, second] = edge;
          const railing = [
            point(...positions[first], heights[first] + 0.55),
            point(...positions[second], heights[second] + 0.55),
            point(...positions[second], heights[second] + 0.7),
            point(...positions[first], heights[first] + 0.7),
          ] as [Vertex, Vertex, Vertex, Vertex];
          railing[0].height = heights[first];
          railing[1].height = heights[second];
          railing[2].height = heights[second];
          railing[3].height = heights[first];
          addQuad(railing, "path-railing", x, y, color, undefined, undefined, undefined, undergroundLevel, undergroundLevel === undefined ? 0 : UNDERGROUND_CONTENT_DEPTH_BIAS);
        }
      }
    }
  }
  for (const sprite of world.sprites ?? []) {
    if (undergroundLevel !== undefined && sprite.height > undergroundLevel) continue;
    const frame = frameFor(atlas, sprite.sprite, sprite.view, sprite.animation, sprite.frame, scale);
    const anchor = point(sprite.x, sprite.y, sprite.height);
    const spriteScale = camera.zoom * (sprite.scale ?? 1);
    const left = anchor.x - frame.anchor.x * spriteScale; const top = anchor.y - frame.anchor.y * spriteScale;
    const right = left + frame.width * spriteScale; const bottom = top + frame.height * spriteScale;
    const spriteHeight = frame.height * (sprite.scale ?? 1) / 8;
    const vertices = [[left,top], [right,top], [right,bottom], [left,bottom]].map(([x,y], index) => {
      const heightOffset = index < 2 ? spriteHeight : 0;
      const height = sprite.height + (undergroundLevel === undefined ? 0 : heightOffset);
      const depthOffset = undergroundLevel !== undefined || sprite.heightDepth ? heightOffset / 4 : 0;
      return { x, y, depth: anchor.depth + depthOffset + (undergroundLevel === undefined ? 0 : UNDERGROUND_CONTENT_DEPTH_BIAS), height, u: 0, v: 0 };
    }) as [Vertex,Vertex,Vertex,Vertex];
    const placement = frame.placements[scale]; const size = atlas.pages[scale][placement.page];
    const coordinates = [[0,0],[1,0],[1,1],[0,1]];
    for (let i = 0; i < 4; i++) { vertices[i].u = (placement.x + coordinates[i][0] * placement.width) / size.width; vertices[i].v = (placement.y + coordinates[i][1] * placement.height) / size.height; }
    for (const indices of [[0,1,2],[0,2,3]]) {
      const source = [vertices[indices[0]], vertices[indices[1]], vertices[indices[2]]] as [Vertex, Vertex, Vertex];
      const metadata = { page: placement.page, color: WHITE, kind: "sprite", tileX: Math.floor(sprite.x), tileY: Math.floor(sprite.y), id: sprite.id };
      if (undergroundLevel === undefined) triangles.push({ ...metadata, vertices: source, height: Math.max(...source.map((vertex) => vertex.height)) });
      else appendPolygonTriangles(triangles, clipTriangleAtHeight(source, undergroundLevel, true), metadata);
    }
  }
  return { triangles };
}

export function pickScene(scene: Scene, x: number, y: number, rasterTolerance = 0): Pick | null {
  let selected: Triangle | undefined; let selectedDepth = -Infinity; let selectedHeight = 0;
  const padding = Math.max(0, rasterTolerance);
  for (const triangle of scene.triangles) {
    if (UNPICKABLE_KINDS.has(triangle.kind)) continue;
    const [a,b,c] = triangle.vertices;
    if (x < Math.min(a.x, b.x, c.x) - padding || x > Math.max(a.x, b.x, c.x) + padding
      || y < Math.min(a.y, b.y, c.y) - padding || y > Math.max(a.y, b.y, c.y) + padding) continue;
    const denominator = (b.y-c.y)*(a.x-c.x)+(c.x-b.x)*(a.y-c.y);
    if (denominator === 0) continue;
    let first = ((b.y-c.y)*(x-c.x)+(c.x-b.x)*(y-c.y))/denominator;
    let second = ((c.y-a.y)*(x-c.x)+(a.x-c.x)*(y-c.y))/denominator;
    let third = 1-first-second;
    if (first < 0 || second < 0 || third < 0) {
      if (rasterTolerance <= 0) continue;
      const vertices = [a, b, c];
      let nearestDistance = Infinity; let nearestWeights = [0, 0, 0];
      for (const [start, end] of [[0, 1], [1, 2], [2, 0]]) {
        const p = vertices[start], q = vertices[end];
        const dx = q.x - p.x, dy = q.y - p.y;
        const lengthSquared = dx * dx + dy * dy;
        const fraction = lengthSquared === 0 ? 0 : Math.max(0, Math.min(1, ((x - p.x) * dx + (y - p.y) * dy) / lengthSquared));
        const distance = (x - p.x - fraction * dx) ** 2 + (y - p.y - fraction * dy) ** 2;
        if (distance < nearestDistance) {
          nearestDistance = distance; nearestWeights = [0, 0, 0];
          nearestWeights[start] = 1 - fraction; nearestWeights[end] = fraction;
        }
      }
      if (nearestDistance > rasterTolerance * rasterTolerance) continue;
      [first, second, third] = nearestWeights;
    }
    const depth = first*a.depth + second*b.depth + third*c.depth;
    if (depth >= selectedDepth) { selected = triangle; selectedDepth = depth; selectedHeight = first*a.height + second*b.height + third*c.height; }
  }
  if (!selected) return null;
  if (selected.id !== undefined) return { kind: selected.kind, id: selected.id, x: selected.tileX, y: selected.tileY, height: selectedHeight };
  const kind = selected.kind === "path-railing" ? "path" : selected.kind as "terrain" | "water" | "path" | "cliff";
  return { kind, x: selected.tileX, y: selected.tileY, height: selectedHeight };
}
