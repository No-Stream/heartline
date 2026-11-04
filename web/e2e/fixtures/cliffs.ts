import type { Rotation } from "../../src/camera";
import { createWorldRenderer, loadAtlas, type WorldView } from "../../src/render";

declare global {
  interface Window {
    renderCliffs: (rotation: Rotation) => Promise<void>;
  }
}

const canvas = document.createElement("canvas");
canvas.width = 640;
canvas.height = 480;
document.body.append(canvas);
const { atlas, images } = await loadAtlas("/atlas.json");
const renderer = createWorldRenderer(canvas, atlas, images);
const surfaces = new Int32Array(4 * 4 * 6);
surfaces.set([3, 3, 3, 3, 0, 0], (1 * 4 + 1) * 6);
surfaces.set([1, 1, 1, 1, 0, 0], (2 * 4 + 2) * 6);
const world: WorldView = { dimensions: new Uint16Array([4, 4]), surfaces, paths: new Int32Array() };
window.renderCliffs = async (rotation) => {
  renderer.render({ world: world, camera: { centerX: 2, centerY: 2, zoom: 2, rotation } });
  renderer.waitForGpu();
};
