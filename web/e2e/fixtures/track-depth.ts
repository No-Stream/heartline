import { projectWorld, type Rotation } from "../../src/camera";
import { createWorldRenderer, type Atlas, type WorldView } from "../../src/render";
import { buildTrackQuads, type TrackStyle } from "../../src/render/track/geometry";

declare global {
  interface Window {
    trackDepthCases(rotation: Rotation): Promise<{ trackCrossing: string[]; flatDepthCrossing: string[]; path: string[]; tunnel: string[] }>;
  }
}

type Vector = [number, number, number];
const style: TrackStyle = { steel: "#b84b62", wood: "#91593d", train: "#2e7075", ink: "#211519", lineWeight: 1 };

window.trackDepthCases = async (rotation) => {
  const texture = document.createElement("canvas"); texture.width = 48; texture.height = 16;
  const context = texture.getContext("2d");
  if (!context) throw new Error("Canvas 2D is unavailable");
  for (const [index, color] of ["#58ad56", "#d3b27a", "#795438"].entries()) {
    context.fillStyle = color; context.fillRect(index * 16, 0, 16, 16);
  }
  const image = new Image(); image.src = texture.toDataURL(); await image.decode();
  const frame = (x: number) => ({ width: 16, height: 16, anchor: { x: 8, y: 8 }, placements: { "1": { page: 0, x, y: 0, width: 16, height: 16 } } });
  const atlas: Atlas = { pages: { "1": [{ file: "synthetic.png", width: 48, height: 16 }] }, frames: {
    "ground/grass_light_0/any/static/0": frame(0),
    ...Object.fromEntries(Array.from({ length: 16 }, (_, index) => [`ground/path_${index.toString(2).padStart(4, "0")}/any/static/0`, frame(16)])),
    ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
      [`ground/cliff_${side}_${variant}/any/static/0`, frame(32)],
      [`ground/cliff_step_${side}_${variant}/any/static/0`, frame(32)],
    ]))),
  } };
  const canvas = document.createElement("canvas"); canvas.width = 800; canvas.height = 600; document.body.append(canvas);
  const renderer = createWorldRenderer(canvas, atlas, { "1": [image] });
  const camera = { centerX: 4.5, centerY: 4.5, zoom: 3, rotation };
  // Express test vectors in camera coordinates so the same geometry is exercised in every view.
  const unrotate = ([x, y, z]: Vector): Vector => {
    const [rx, ry] = [[x, y], [y, -x], [-x, -y], [-y, x]][rotation];
    return [rx, ry, z];
  };
  const position = (point: Vector): Vector => { const [x, y, z] = unrotate(point); return [x + 4.5, y + 4.5, z]; };
  const add = (a: Vector, b: Vector): Vector => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
  const rail = (id: number, start: Vector, end: Vector, lateral: Vector): number[] => [id, ...start, ...lateral, 0, 0, 1, ...end, ...lateral, 0, 0, 1];
  const flat = new Int32Array(8 * 8 * 6);
  const world = (surfaces: Int32Array, paths = new Int32Array(), carved = new Int32Array()): WorldView => ({ dimensions: new Uint16Array([8, 8]), surfaces, paths, carved });
  const render = (view: WorldView, rails: number[], flattenDepth = false): void => renderer.render(view, camera, 0, [(renderCamera, viewport) => {
    const quads = buildTrackQuads(new Float32Array(rails), new Float32Array(), new Float32Array(), new Float32Array(), renderCamera, viewport, style);
    if (!flattenDepth) return quads;
    const depths = new Map<string, number>();
    return quads.map((quad) => {
      if (!depths.has(quad.id)) depths.set(quad.id, quad.vertices[0].depth);
      return { ...quad, vertices: quad.vertices.map((vertex) => ({ ...vertex, depth: depths.get(quad.id)! })) as typeof quad.vertices };
    });
  }]);
  const pick = (point: Vector): string => {
    const screen = projectWorld(camera, canvas, { x: point[0], y: point[1], z: point[2] });
    const result = renderer.pick(screen.x, screen.y);
    return result && "id" in result ? result.id : result?.kind ?? "none";
  };

  const start: Vector = [-1.5, -1, 2]; const end: Vector = [1.5, -1, 2];
  const lateral = unrotate([0, 1, 0]);
  const crossingRails = [
    ...rail(1, position(add(start, [-0.8, -0.8, -0.8])), position(add(end, [0.8, 0.8, 0.8])), lateral),
    ...rail(2, position(start), position(end), lateral),
  ];
  const crossingPicks = () => [0.2, 0.8].map((fraction) => pick(position([start[0] + (end[0] - start[0]) * fraction, start[1], start[2]])));
  render(world(flat), crossingRails);
  const trackCrossing = crossingPicks();
  render(world(flat), crossingRails, true);
  const flatDepthCrossing = crossingPicks();

  const pathWorld = world(flat, new Int32Array([4, 4, 0, 0]));
  const trackAt = (id: number, center: Vector) => rail(id, position(add(center, [-0.8, 0.8, 0])), position(add(center, [0.8, -0.8, 0])), unrotate([1, 1, 0]));
  const path: string[] = [];
  for (const offset of [0.5, -0.5]) {
    render(pathWorld, trackAt(3, [offset, offset, offset]));
    path.push(pick(position([0, 0, 0])));
  }

  const high = new Int32Array(8 * 8 * 6);
  for (let index = 0; index < high.length; index += 6) high.set([8, 8, 8, 8, 0, 0], index);
  const carved: number[] = [];
  for (let y = 3; y <= 5; y++) for (let x = 3; x <= 5; x++) carved.push(x, y, 0, 8);
  const tunnel: string[] = [];
  for (const spans of [new Int32Array(), new Int32Array(carved)]) {
    render(world(high, new Int32Array(), spans), trackAt(4, [-1, -1, 1]));
    tunnel.push(pick(position([0, 0, 2])));
  }
  return { trackCrossing, flatDepthCrossing, path, tunnel };
};
