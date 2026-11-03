import { projectWorld, unprojectGround, type Camera, type Viewport } from "../camera";

export interface AtlasFrame {
  width: number; height: number; anchor: { x: number; y: number };
  properties?: Record<string, string>;
  placements: Record<string, { page: number; x: number; y: number; width: number; height: number }>;
}
export interface Atlas {
  pages: Record<string, { file: string; width: number; height: number }[]>;
  frames: Record<string, AtlasFrame>;
}
export interface WorldSprite { id: string; sprite: string; x: number; y: number; height: number; view?: string; animation?: string; frame?: number }
export interface WorldView { dimensions: Uint16Array; surfaces: Int32Array; paths: Int32Array; carved?: Int32Array; sprites?: readonly WorldSprite[] }
export type Pick = { kind: "terrain" | "water" | "path" | "cliff"; x: number; y: number; height: number } | { kind: string; id: string; x: number; y: number; height: number };
export interface Vertex { x: number; y: number; depth: number; height: number; u: number; v: number }
export interface Triangle { vertices: [Vertex, Vertex, Vertex]; page: number; color: readonly [number, number, number, number]; kind: string; tileX: number; tileY: number; height: number; id?: string }
export interface Scene { triangles: Triangle[] }
export interface GeometryQuad { vertices: [Vertex, Vertex, Vertex, Vertex]; page?: number; color: Triangle["color"]; kind: string; id: string; tileX: number; tileY: number }
const WHITE = [1, 1, 1, 1] as const;
const QUAD_TRIANGLES = [[0, 1, 2], [0, 2, 3]] as const;

export function appendGeometry(scene: Scene, quads: readonly GeometryQuad[], viewport?: Viewport): void {
  for (const quad of quads) {
    if (viewport && (quad.vertices.every((vertex) => vertex.x < 0) || quad.vertices.every((vertex) => vertex.x > viewport.width) || quad.vertices.every((vertex) => vertex.y < 0) || quad.vertices.every((vertex) => vertex.y > viewport.height))) continue;
    for (const indices of QUAD_TRIANGLES) scene.triangles.push({
      vertices: indices.map((index) => quad.vertices[index]) as Triangle["vertices"],
      page: quad.page ?? -1, color: quad.color, kind: quad.kind, id: quad.id,
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

export function buildScene(world: WorldView, camera: Camera, viewport: Viewport, atlas: Atlas, scale = "1", waterFrame = 0): Scene {
  const [width, height] = world.dimensions;
  if (!width || !height || world.dimensions.length !== 2 || world.surfaces.length !== width * height * 6 || world.paths.length % 4 !== 0) throw new Error("Invalid world snapshot");
  const triangles: Triangle[] = [];
  const grass = frameFor(atlas, "ground/grass_light_0", "any", "static", 0, scale);
  const paths = new Map<number, [number, number][]>();
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
    const key = y * width + x;
    paths.set(key, [...(paths.get(key) ?? []), [z, slope]]);
  }
  const pathCorners = (z: number, slope: number): number[] =>
    [[0,0,0,0], [1,1,0,0], [0,1,1,0], [0,0,1,1], [1,0,0,1]][slope].map((offset) => z + offset);
  const point = (x: number, y: number, heightSteps: number): Vertex => {
    const projected = projectWorld(camera, viewport, { x, y, z: heightSteps / 4 });
    const rotated = [x + y, x - y, -x - y, -x + y][camera.rotation];
    return { ...projected, depth: rotated + heightSteps / 4, height: heightSteps, u: 0, v: 0 };
  };
  const addQuad = (corners: [Vertex, Vertex, Vertex, Vertex], kind: Triangle["kind"], x: number, y: number, color: Triangle["color"], frame?: AtlasFrame, id?: string, textureCoordinates?: number[][]): void => {
    const vertices = corners.map((corner) => ({ ...corner })) as [Vertex, Vertex, Vertex, Vertex];
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
    for (const indices of QUAD_TRIANGLES) triangles.push({ vertices: [vertices[indices[0]], vertices[indices[1]], vertices[indices[2]]], page, color, kind, tileX: x, tileY: y, height: Math.max(...vertices.map((vertex) => vertex.height)), id });
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
  const bounds = [minHeight, maxHeight].flatMap((heightSteps) => [
    { x: 0, y: 0 }, { x: viewport.width, y: 0 }, { x: viewport.width, y: viewport.height }, { x: 0, y: viewport.height },
  ].map((screen) => unprojectGround(camera, viewport, screen, heightSteps / 4)));
  const startX = Math.max(0, Math.floor(Math.min(...bounds.map((point) => point.x))) - 2);
  const endX = Math.min(width, Math.ceil(Math.max(...bounds.map((point) => point.x))) + 2);
  const startY = Math.max(0, Math.floor(Math.min(...bounds.map((point) => point.y))) - 2);
  const endY = Math.min(height, Math.ceil(Math.max(...bounds.map((point) => point.y))) + 2);
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
    if (maxX < -100 || minX > viewport.width + 100 || maxY < -100 || minY > viewport.height + 100) continue;
    const opening = (carved.get(y * width + x) ?? []).find(([, top]) => corners.every((corner) => top >= corner));
    if (!opening) addQuad(terrain, "terrain", x, y, WHITE, grass);
    const exposedCorners = opening ? corners.map((corner) => Math.min(corner, opening[0])) : corners;
    for (let edge = 0; edge < 4; edge++) {
      const next = (edge + 1) % 4;
      const dx = [0,1,0,-1][edge]; const dy = [-1,0,1,0][edge];
      const nx = x + dx; const ny = y + dy;
      const neighborIndex = (ny * width + nx) * 6;
      const adjacent = nx >= 0 && ny >= 0 && nx < width && ny < height ? [world.surfaces[neighborIndex + [3,0,1,2][edge]], world.surfaces[neighborIndex + [2,3,0,1][edge]]] : [0,0];
      const mapEdge = nx < 0 || ny < 0 || nx >= width || ny >= height;
      if (!mapEdge) {
        const neighborCorners = world.surfaces.subarray(neighborIndex, neighborIndex + 4);
        const neighborOpening = (carved.get(ny * width + nx) ?? []).find(([, top]) => neighborCorners.every((corner) => top >= corner));
        if (neighborOpening) {
          adjacent[0] = Math.min(adjacent[0], neighborOpening[0]);
          adjacent[1] = Math.min(adjacent[1], neighborOpening[0]);
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
        addQuad([point(...positions[edge], topFirst), point(...positions[next], topNext), point(...positions[next], bottomNext), point(...positions[edge], bottomFirst)], "cliff", x, y, WHITE, face, undefined, uv);
      }
    }
    if (world.surfaces[index + 4]) {
      const level = world.surfaces[index + 5];
      if (corners.some((corner) => corner < level)) addQuad(quad([level,level,level,level]), "water", x, y, WHITE, frameFor(atlas, "ground/water", "any", "loop", waterFrame, scale));
    }
    for (const path of paths.get(y * width + x) ?? []) {
      const [z, slope] = path;
      const heights = pathCorners(z, slope);
      if (heights.some((height, corner) => height < corners[corner]) &&
          (!opening || heights.some((height) => height < opening[0] || height >= opening[1]))) continue;
      const pathQuad = quad(heights);
      for (const vertex of pathQuad) vertex.depth += 0.001;
      const directions = [
        { dx: 1, dy: 0, edge: [1, 2], neighborEdge: [0, 3] },
        { dx: 0, dy: 1, edge: [3, 2], neighborEdge: [0, 1] },
        { dx: -1, dy: 0, edge: [0, 3], neighborEdge: [1, 2] },
        { dx: 0, dy: -1, edge: [0, 1], neighborEdge: [3, 2] },
      ];
      const connections = [0, 1, 2, 3].map((direction) => {
        const { dx, dy, edge, neighborEdge } = directions[(direction - camera.rotation + 4) % 4];
        const nx = x + dx; const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= width || ny >= height) return "0";
        return (paths.get(ny * width + nx) ?? []).some(([neighborHeight, neighborSlope]) => {
          const neighbor = pathCorners(neighborHeight, neighborSlope);
          return heights[edge[0]] === neighbor[neighborEdge[0]] && heights[edge[1]] === neighbor[neighborEdge[1]];
        }) ? "1" : "0";
      }).join("");
      addQuad(pathQuad, "path", x, y, WHITE, frameFor(atlas, `ground/path_${connections}`, "any", "static", 0, scale));
    }
  }
  for (const sprite of world.sprites ?? []) {
    const frame = frameFor(atlas, sprite.sprite, sprite.view, sprite.animation, sprite.frame, scale);
    const anchor = point(sprite.x, sprite.y, sprite.height);
    const left = anchor.x - frame.anchor.x * camera.zoom; const top = anchor.y - frame.anchor.y * camera.zoom;
    const right = left + frame.width * camera.zoom; const bottom = top + frame.height * camera.zoom;
    const vertices = [[left,top], [right,top], [right,bottom], [left,bottom]].map(([x,y]) => ({ x,y,depth:anchor.depth,height:sprite.height,u:0,v:0 })) as [Vertex,Vertex,Vertex,Vertex];
    const placement = frame.placements[scale]; const size = atlas.pages[scale][placement.page];
    const coordinates = [[0,0],[1,0],[1,1],[0,1]];
    for (let i = 0; i < 4; i++) { vertices[i].u = (placement.x + coordinates[i][0] * placement.width) / size.width; vertices[i].v = (placement.y + coordinates[i][1] * placement.height) / size.height; }
    for (const indices of [[0,1,2],[0,2,3]]) triangles.push({ vertices: [vertices[indices[0]],vertices[indices[1]],vertices[indices[2]]], page: placement.page, color: WHITE, kind: "sprite", tileX: Math.floor(sprite.x), tileY: Math.floor(sprite.y), height: sprite.height, id: sprite.id });
  }
  return { triangles };
}

export function pickScene(scene: Scene, x: number, y: number, rasterTolerance = 0): Pick | null {
  let selected: Triangle | undefined; let selectedDepth = -Infinity; let selectedHeight = 0;
  for (const triangle of scene.triangles) {
    const [a,b,c] = triangle.vertices;
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
  return { kind: selected.kind as "terrain" | "water" | "path" | "cliff", x: selected.tileX, y: selected.tileY, height: selectedHeight };
}
