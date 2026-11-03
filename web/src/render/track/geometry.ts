import { projectWorld, type Camera, type Viewport } from "../../camera";
import type { Atlas, Triangle, Vertex } from "../scene";

type Vec = [number, number, number];
export interface TrackQuad { vertices: [Vertex, Vertex, Vertex, Vertex]; color: Triangle["color"]; kind: "track" | "support" | "train" | "rider"; id: string; tileX: number; tileY: number; page?: number }
export interface TrackStyle { steel: string; wood: string; train: string; ink: string; lineWeight: number }
const add = (a: Vec, b: Vec): Vec => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: Vec, n: number): Vec => [a[0] * n, a[1] * n, a[2] * n];
const cross = (a: Vec, b: Vec): Vec => [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
const normalize = (a: Vec): Vec => { const length = Math.hypot(...a); if (length < 1e-8) throw new Error("Degenerate coaster geometry"); return scale(a, 1 / length); };
const vector = (data: Float32Array, offset: number): Vec => [data[offset], data[offset+1], data[offset+2]];
const color = (hex: string): Triangle["color"] => [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, 1];
export function trackStyle(atlas: Atlas): TrackStyle {
  const { themeStyle } = atlas;
  if (!themeStyle || !themeStyle.steel || !themeStyle.wood || !themeStyle.train || !themeStyle.ink) throw new Error("Atlas is missing coaster theme style");
  if (!Number.isFinite(themeStyle.lineWeight) || themeStyle.lineWeight <= 0) throw new Error("Invalid coaster line weight");
  return themeStyle;
}

export function interpolateTrainPoses(previous: Float32Array, current: Float32Array, interpolation: number): Float32Array {
  if (current.length % 11 || previous.length % 11 || interpolation < 0 || interpolation > 1) throw new Error("Invalid train snapshot");
  const result = new Float32Array(current);
  for (let i = 0; i < current.length; i += 11) {
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

export function buildTrackQuads(rails: Float32Array, ties: Float32Array, supports: Float32Array, poses: Float32Array, camera: Camera, viewport: Viewport, style: TrackStyle): TrackQuad[] {
  if (rails.length % 19 || ties.length % 13 || supports.length % 7 || poses.length % 11) throw new Error("Invalid coaster snapshot");
  const quads: TrackQuad[] = [];
  const project = (point: Vec): Vertex => ({ ...projectWorld(camera, viewport, { x: point[0], y: point[1], z: point[2] }), depth: [point[0]+point[1], point[0]-point[1], -point[0]-point[1], -point[0]+point[1]][camera.rotation] + point[2], height: point[2]*4, u: 0, v: 0 });
  const box = (corners: Vec[], kind: TrackQuad["kind"], id: string, fillColor: TrackQuad["color"], openEnds = false) => {
    const faces = [[0,1,2,3], [4,7,6,5], [0,4,5,1], [1,5,6,2], [2,6,7,3], [3,7,4,0]];
    for (const face of openEnds ? faces.slice(2) : faces) {
      const faceCorners = face.map((i) => corners[i]);
      const append = (points: Vec[], faceColor: TrackQuad["color"]) => quads.push({ vertices: points.map(project) as TrackQuad["vertices"], color: faceColor, kind, id, tileX: Math.floor(corners[0][0]), tileY: Math.floor(corners[0][1]) });
      const inkWidth = (openEnds ? 0.012 : 0.008) * style.lineWeight;
      const edgeLength = (first: Vec, second: Vec) => Math.hypot(...add(first, scale(second, -1)));
      const horizontalInset = openEnds ? 0 : Math.min(0.18, inkWidth / edgeLength(faceCorners[0], faceCorners[1]));
      const verticalInset = Math.min(openEnds ? 0.25 : 0.18, inkWidth / edgeLength(faceCorners[0], faceCorners[3]));
      const inner = faceCorners.map((corner, index) => {
        const nextInset = index % 2 === 0 ? horizontalInset : verticalInset;
        const previousInset = index % 2 === 0 ? verticalInset : horizontalInset;
        return add(add(scale(corner, (1-nextInset)*(1-previousInset)), scale(faceCorners[(index+1)%4], nextInset*(1-previousInset))), add(scale(faceCorners[(index+3)%4], previousInset*(1-nextInset)), scale(faceCorners[(index+2)%4], nextInset*previousInset)));
      });
      append(inner, fillColor);
      for (const edge of openEnds ? [0, 2] : [0, 1, 2, 3]) {
        const next = (edge+1)%4;
        append([faceCorners[edge], faceCorners[next], inner[next], inner[edge]], color(style.ink));
      }
    }
  };
  const beam = (start: Vec, end: Vec, sideStart: Vec, sideEnd: Vec, upStart: Vec, upEnd: Vec, width: number, kind: TrackQuad["kind"], id: string, color: TrackQuad["color"], openEnds = false) => {
    const ring = (center: Vec, side: Vec, up: Vec): Vec[] => [[-1,-1], [1,-1], [1,1], [-1,1]].map(([s,u]) => add(add(center, scale(side, s*width)), scale(up, u*width)));
    box([...ring(start, sideStart, upStart), ...ring(end, sideEnd, upEnd)], kind, id, color, openEnds);
  };
  const steel = color(style.steel); const wood = color(style.wood);
  for (let i = 0; i < rails.length; i += 19) {
    beam(vector(rails, i+1), vector(rails, i+10), normalize(vector(rails, i+4)), normalize(vector(rails, i+13)), normalize(vector(rails, i+7)), normalize(vector(rails, i+16)), 0.035, "track", `track-${rails[i]}`, steel, true);
  }
  for (let i = 0; i < ties.length; i += 13) {
    const up = normalize(vector(ties, i+7)), tangent = normalize(vector(ties, i+10));
    beam(vector(ties, i+1), vector(ties, i+4), up, up, tangent, tangent, 0.025, "track", `track-${ties[i]}`, wood);
  }
  for (let i = 0; i < supports.length; i += 7) {
    const start = vector(supports, i+1), end = vector(supports, i+4);
    const direction = normalize(add(end, scale(start, -1)));
    const side = normalize(cross(direction, Math.abs(direction[2]) > 0.9 ? [1,0,0] : [0,0,1]));
    const up = normalize(cross(direction, side));
    beam(start, end, side, side, up, up, 0.045, "support", `support-${supports[i]}`, wood);
    if (start[0] === end[0] && start[1] === end[1] && end[2] > start[2]) {
      beam(start, add(start, [0, 0, 0.05]), [1, 0, 0], [1, 0, 0], [0, 1, 0], [0, 1, 0], 0.1, "support", `footer-${supports[i]}`, wood);
    }
  }
  for (let i = 0; i < poses.length; i += 11) {
    const center = vector(poses, i+1), forward = normalize(vector(poses, i+4)), up = normalize(vector(poses, i+7));
    const side = normalize(cross(up, forward));
    const prism = (centerPoint: Vec, length: number, width: number, height: number, kind: TrackQuad["kind"], id: string, color: TrackQuad["color"]) => {
      const ring = (along: number): Vec[] => [[-1,-1], [1,-1], [1,1], [-1,1]].map(([s,u]) => add(add(add(centerPoint, scale(forward, along*length)), scale(side, s*width)), scale(up, u*height)));
      box([...ring(-1), ...ring(1)], kind, id, color);
    };
    prism(add(center, scale(up, 0.12)), 0.29, 0.21, 0.12, "train", `train-${poses[i]}`, color(style.train));
  }
  return quads;
}

export function buildRiderQuads(poses: Float32Array, camera: Camera, viewport: Viewport, atlas: Atlas, scaleName: string): TrackQuad[] {
  if (poses.length % 11) throw new Error("Invalid train snapshot");
  const riders: TrackQuad[] = [];
  const views = ["se", "sw", "nw", "ne"] as const;
  const cameraDirection: Vec = [[1,1,1], [1,-1,1], [-1,-1,1], [-1,1,1]][camera.rotation] as Vec;
  for (let i = 0; i < poses.length; i += 11) {
    const center = vector(poses, i+1), forward = normalize(vector(poses, i+4)), up = normalize(vector(poses, i+7));
    const side = normalize(cross(up, forward));
    const cameraRight = cross(cameraDirection, up);
    const spriteRight = Math.hypot(...cameraRight) < 1e-8 ? side : normalize(cameraRight);
    const heading = Math.abs(forward[0]) >= Math.abs(forward[1]) ? (forward[0] >= 0 ? 0 : 2) : (forward[1] >= 0 ? 1 : 3);
    for (let seat = 0; seat < 4; seat++) if ((poses[i+10] & (1 << seat)) !== 0) {
      const anchor = add(add(add(center, scale(forward, seat < 2 ? -0.14 : 0.14)), scale(side, seat % 2 ? 0.1 : -0.1)), scale(up, 0.2));
      const view = views[(heading + camera.rotation) % 4];
      const frame = atlas.frames[`people/${["maya", "otis", "priya", "gus"][seat]}/${view}/idle/0`];
      const placement = frame?.placements[scaleName];
      if (!placement) throw new Error("Atlas is missing rider frame");
      const page = atlas.pages[scaleName][placement.page];
      const factor = 0.012;
      const left = -frame.anchor.x * factor, right = (frame.width - frame.anchor.x) * factor;
      const top = frame.anchor.y * factor, bottom = (frame.anchor.y - frame.height) * factor;
      const points: Vec[] = [[left,top], [right,top], [right,bottom], [left,bottom]].map(([across, vertical]) => add(add(anchor, scale(spriteRight, across)), scale(up, vertical)));
      const uv = [[0,0], [1,0], [1,1], [0,1]];
      const vertices = points.map((point, index) => ({ ...projectWorld(camera, viewport, { x: point[0], y: point[1], z: point[2] }), depth: [point[0]+point[1], point[0]-point[1], -point[0]-point[1], -point[0]+point[1]][camera.rotation] + point[2], height: point[2]*4, u: (placement.x + uv[index][0]*placement.width)/page.width, v: (placement.y + uv[index][1]*placement.height)/page.height })) as TrackQuad["vertices"];
      riders.push({ vertices, color: [1,1,1,1], kind: "rider", id: `rider-${poses[i]}-${seat}`, tileX: Math.floor(anchor[0]), tileY: Math.floor(anchor[1]), page: placement.page });
    }
  }
  return riders;
}
