import { projectWorld, type Rotation } from "../../src/camera";
import { createWorldRenderer, type Atlas, type WorldView } from "../../src/render";

declare global { interface Window { batchFixture(rotation: Rotation): Promise<{ ids: (string | undefined)[]; colors: number[][] }> } }

window.batchFixture = async (rotation) => {
  const images = await Promise.all(["#808080", "#ff0000", "#0000ff"].map(async (color) => {
    const canvas = document.createElement("canvas"); canvas.width = 16; canvas.height = 16;
    const context = canvas.getContext("2d")!; context.fillStyle = color; context.fillRect(0, 0, 16, 16);
    const image = new Image(); image.src = canvas.toDataURL(); await image.decode(); return image;
  }));
  const frame = (page: number) => ({ width: 16, height: 16, anchor: { x: 8, y: 8 }, placements: { "1": { page, x: 0, y: 0, width: 16, height: 16 } } });
  const atlas: Atlas = { pages: { "1": images.map(() => ({ file: "synthetic.png", width: 16, height: 16 })) }, frames: {
    "ground/grass_light_0/any/static/0": frame(0),
    ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [[`ground/cliff_${side}_${variant}/any/static/0`, frame(0)], [`ground/cliff_step_${side}_${variant}/any/static/0`, frame(0)]]))),
    "red/any/static/0": frame(1), "blue/any/static/0": frame(2),
  } };
  const canvas = document.createElement("canvas"); canvas.width = 800; canvas.height = 800; document.body.append(canvas);
  const renderer = createWorldRenderer(canvas, atlas, { "1": images });
  const sprites = [2, 4, 6].map((coordinate, index) => ({ id: `sprite-${index}`, sprite: index === 1 ? "blue" : "red", x: coordinate + 0.5, y: coordinate + 0.5, height: 0 }));
  const world: WorldView = { dimensions: new Uint16Array([9, 9]), surfaces: new Int32Array(9 * 9 * 6), paths: new Int32Array(), sprites };
  const camera = { centerX: 4.5, centerY: 4.5, zoom: 2, rotation };
  renderer.render(world, camera);
  const gl = canvas.getContext("webgl2")!;
  const ids: (string | undefined)[] = []; const colors: number[][] = [];
  for (const sprite of sprites) {
    const anchor = projectWorld(camera, canvas, { x: sprite.x, y: sprite.y, z: 0 });
    const x = Math.floor(anchor.x); const y = Math.floor(anchor.y - 8);
    const pick = renderer.pick(x, y); ids.push(pick?.kind === "sprite" ? pick.id : undefined);
    const pixel = new Uint8Array(4); gl.readPixels(x, canvas.height - 1 - y, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel); colors.push([...pixel]);
  }
  canvas.remove(); return { ids, colors };
};
