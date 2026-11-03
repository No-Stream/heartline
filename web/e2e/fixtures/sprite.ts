import { projectWorld, type Rotation } from "../../src/camera";
import { guestViewForRotation } from "../../src/guest-view";
import { createWorldRenderer, type Atlas, type Pick, type WorldView } from "../../src/render";

declare global {
  interface Window {
    spriteFixture: (rotation: Rotation) => Promise<{ visible: Pick | null; transparent: Pick | null; occluded: Pick | null; color: number[] }>;
  }
}

window.spriteFixture = async (rotation) => {
  const imageCanvas = document.createElement("canvas");
  imageCanvas.width = 80;
  imageCanvas.height = 16;
  const context = imageCanvas.getContext("2d");
  if (!context) throw new Error("Canvas 2D is unavailable");
  context.fillStyle = "#777777";
  context.fillRect(0, 0, 16, 16);
  for (const [index, color] of ["#ff0000", "#00ff00", "#0000ff", "#ffff00"].entries()) {
    context.fillStyle = color;
    context.fillRect(16 + index * 16, 0, 8, 16);
  }
  const image = new Image();
  image.src = imageCanvas.toDataURL();
  await image.decode();
  const frame = (x: number) => ({ width: 16, height: 16, anchor: { x: 8, y: 8 }, placements: { "1": { page: 0, x, y: 0, width: 16, height: 16 } } });
  const atlas: Atlas = { pages: { "1": [{ file: "synthetic.png", width: 80, height: 16 }] }, frames: {
    "ground/grass_light_0/any/static/0": frame(0),
    ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [[`ground/cliff_${side}_${variant}/any/static/0`, frame(0)], [`ground/cliff_step_${side}_${variant}/any/static/0`, frame(0)]]))),
    ...Object.fromEntries(["se", "sw", "nw", "ne"].map((view, index) => [`people/test/${view}/idle/0`, frame(16 + index * 16)])),
  } };
  const canvas = document.createElement("canvas");
  canvas.width = 400;
  canvas.height = 400;
  document.body.append(canvas);
  const renderer = createWorldRenderer(canvas, atlas, { "1": [image] });
  const world: WorldView = { dimensions: new Uint16Array([1, 1]), surfaces: new Int32Array([0, 0, 0, 0, 0, 0]), paths: new Int32Array(), sprites: [
    { id: "controlled-agent", sprite: "people/test", x: 0.5, y: 0.5, height: 0, view: guestViewForRotation(rotation), animation: "idle", frame: 0 },
  ] };
  const camera = { centerX: 0.5, centerY: 0.5, zoom: 3, rotation };
  renderer.render(world, camera);
  const anchor = projectWorld(camera, canvas, { x: 0.5, y: 0.5, z: 0 });
  const visible = renderer.pick(anchor.x - 10, anchor.y - 4);
  const transparent = renderer.pick(anchor.x + 10, anchor.y - 4);
  const occluded = renderer.pick(anchor.x - 10, anchor.y + 4);
  const gl = canvas.getContext("webgl2");
  if (!gl) throw new Error("WebGL2 is unavailable");
  const color = new Uint8Array(4);
  gl.readPixels(anchor.x - 10, canvas.height - 1 - (anchor.y - 4), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, color);
  canvas.remove();
  return { visible, transparent, occluded, color: [...color] };
};
