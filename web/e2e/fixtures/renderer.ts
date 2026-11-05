import { projectWorld, type Rotation } from "../../src/camera";
import { createWorldRenderer, type Atlas, type WorldView } from "../../src/render";
import type { GeometryQuad } from "../../src/render/scene";

interface Sample {
  pick: ReturnType<ReturnType<typeof createWorldRenderer>["pick"]>;
  color: number[];
  pickingPasses: number[];
  updatedPick: ReturnType<ReturnType<typeof createWorldRenderer>["pick"]>;
  resizedPick: ReturnType<ReturnType<typeof createWorldRenderer>["pick"]>;
  regionMatches: boolean;
  clippedRegionMatches: boolean;
}

declare global {
  interface Window {
    renderFixture: (rotation: Rotation, stacked: boolean, hideUpper: boolean, extra?: "behind" | "solid" | "textured" | "thin", checkRegion?: boolean, disturbState?: boolean) => Promise<Sample>;
  }
}

window.renderFixture = async (rotation, stacked, hideUpper, extra, checkRegion = false, disturbState = false) => {
  const imageCanvas = document.createElement("canvas");
  imageCanvas.width = 48;
  imageCanvas.height = 16;
  const context = imageCanvas.getContext("2d");
  if (!context) throw new Error("Canvas 2D is unavailable");
  for (const [index, color] of ["#19a33b", "#c82d36", "#745120"].entries()) {
    context.fillStyle = color;
    context.fillRect(index * 16, 0, 16, 16);
  }
  const image = new Image();
  image.src = imageCanvas.toDataURL();
  await image.decode();
  const frame = (x: number) => ({ width: 16, height: 16, anchor: { x: 8, y: 8 }, placements: { "1": { page: 0, x, y: 0, width: 16, height: 16 } } });
  const atlas: Atlas = {
    pages: { "1": [{ file: "synthetic.png", width: 48, height: 16 }] },
    frames: {
      "ground/grass_light_0/any/static/0": frame(0),
      "ground/path_0000/any/static/0": frame(16),
      "people/test/any/static/0": frame(0),
      "ground/cliff_left_0/any/static/0": frame(32),
      "ground/cliff_right_0/any/static/0": frame(32),
      ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
        [`ground/cliff_${side}_${variant}/any/static/0`, frame(32)],
        [`ground/cliff_step_${side}_${variant}/any/static/0`, frame(32)],
      ]))),
    },
  };
  const canvas = document.createElement("canvas");
  canvas.width = 400;
  canvas.height = 400;
  document.body.append(canvas);
  const gl = canvas.getContext("webgl2");
  if (!gl) throw new Error("WebGL2 is unavailable");
  let pickingDraws = 0;
  const drawArraysInstanced = gl.drawArraysInstanced.bind(gl);
  gl.drawArraysInstanced = (...args) => {
    if (gl.getParameter(gl.FRAMEBUFFER_BINDING)) pickingDraws++;
    drawArraysInstanced(...args);
  };
  const renderer = createWorldRenderer(canvas, atlas, { "1": [image] });
  const upper = [[2, 2], [2, 1], [1, 1], [1, 2]][rotation];
  const lower = [[1, 1], [1, 2], [2, 2], [2, 1]][rotation];
  const upperPath = [...upper, 4, 1];
  const lowerPath = [...lower, 0, 0];
  const paths = hideUpper ? lowerPath : [...lowerPath, ...upperPath];
  const world: WorldView = {
    dimensions: new Uint16Array(stacked ? [4, 4] : [1, 1]),
    surfaces: stacked ? new Int32Array(4 * 4 * 6) : new Int32Array([0, 0, 1, 1, 0, 0]),
    paths: new Int32Array(stacked ? paths : []),
    sprites: extra ? [{ id: "guest", sprite: "people/test", x: 0.5, y: 0.5, height: 4 }] : [],
  };
  const camera = { centerX: stacked ? 2 : 0.5, centerY: stacked ? 2 : 0.5, zoom: 3, rotation };
  const screen = projectWorld(camera, canvas, { x: stacked ? upper[0] + 0.5 : 0.5, y: stacked ? upper[1] + 0.5 : 0.5, z: stacked ? 1.125 : extra ? 1 : 0.125 });
  const disturb = () => {
    if (!disturbState) return;
    gl.depthMask(false); gl.depthFunc(gl.NEVER); gl.enable(gl.DITHER);
    gl.disable(gl.BLEND); gl.disable(gl.DEPTH_TEST); gl.viewport(0, 0, 1, 1);
  };
  disturb();
  renderer.render({ world: world, camera: camera, waterFrame: 0, geometryPasses: extra ? [() => [{
    vertices: (extra === "thin" ? [[0.516,-8], [0.696,-8], [0.696,8], [0.516,8]] : [[-8,-8], [8,-8], [8,8], [-8,8]]).map(([x, y]) => ({ x: (extra === "thin" ? Math.floor(screen.x) : screen.x) + x, y: screen.y + y, depth: extra === "behind" ? -100 : 100, height: 2, u: 0.5, v: 0.5 })) as GeometryQuad["vertices"],
    kind: "fixture", id: "extra", tileX: 0, tileY: 0,
    color: extra === "textured" ? [1, 1, 1, 1] : [0, 0, 1, 1], page: extra === "textured" ? 0 : undefined,
  }]] : [] });
  const pickingPasses = [pickingDraws];
  const color = new Uint8Array(4);
  gl.readPixels(Math.floor(screen.x), canvas.height - 1 - Math.floor(screen.y), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, color);
  disturb();
  const pick = renderer.pick(screen.x, screen.y);
  const compareRegion = (x: number, y: number, width: number, height: number): boolean => {
    const region = renderer.pickRegion(x, y, width, height);
    return region.every((value, index) => JSON.stringify(value) === JSON.stringify(renderer.pick(x + index % width, y + Math.floor(index / width))));
  };
  let regionMatches = true; let clippedRegionMatches = true;
  if (checkRegion) {
    regionMatches = compareRegion(Math.floor(screen.x) - 10, Math.floor(screen.y) - 10, 21, 21);
    clippedRegionMatches = compareRegion(-2, -2, 4, 4);
  }
  pickingPasses.push(pickingDraws);
  renderer.pick(screen.x, screen.y);
  pickingPasses.push(pickingDraws);
  renderer.render({ world: { ...world, sprites: [], paths: new Int32Array() }, camera: camera });
  pickingPasses.push(pickingDraws);
  const terrainScreen = projectWorld(camera, canvas, { x: stacked ? upper[0] + 0.5 : 0.5, y: stacked ? upper[1] + 0.5 : 0.5, z: stacked ? 0 : 0.125 });
  const updatedPick = renderer.pick(terrainScreen.x, terrainScreen.y);
  pickingPasses.push(pickingDraws);
  canvas.width = 500;
  renderer.render({ world: { ...world, sprites: [], paths: new Int32Array() }, camera: camera });
  pickingPasses.push(pickingDraws);
  const resizedScreen = projectWorld(camera, canvas, { x: stacked ? upper[0] + 0.5 : 0.5, y: stacked ? upper[1] + 0.5 : 0.5, z: stacked ? 0 : 0.125 });
  const resizedPick = renderer.pick(resizedScreen.x, resizedScreen.y);
  if (checkRegion) regionMatches &&= compareRegion(Math.floor(resizedScreen.x) - 10, Math.floor(resizedScreen.y) - 10, 21, 21);
  pickingPasses.push(pickingDraws);
  canvas.remove();
  return { pick, color: [...color], pickingPasses, updatedPick, resizedPick, regionMatches, clippedRegionMatches };
};
