import init, { WasmSimulation } from "../../src/wasm/theme_park_sim_wasm.js";
import { createWorldRenderer, loadAtlas } from "../../src/render";
import { buildPeople } from "../../src/render/people";
import { captureWorldSnapshot } from "../../src/world-snapshot";
import { projectWorld } from "../../src/camera";

declare global {
  interface Window {
    renderGuestFixture(): Promise<string[]>;
  }
}

window.renderGuestFixture = async () => {
  await init();
  const simulation = new WasmSimulation(2026n, 12);
  for (let tick = 0; tick < 360; tick++) simulation.tick();
  const { atlas, images } = await loadAtlas("/atlas.json");
  const canvas = document.createElement("canvas");
  canvas.width = 800;
  canvas.height = 500;
  document.body.append(canvas);
  const renderer = createWorldRenderer(canvas, atlas, images);
  const guests = buildPeople(undefined, { guestIds: simulation.guest_ids(), guests: simulation.snapshot(), appearances: simulation.guest_appearances() }, 1, 0, atlas);
  const camera = { centerX: 231, centerY: 256.5, zoom: 1.5, rotation: 0 as const };
  renderer.render({ ...captureWorldSnapshot(simulation).message, sprites: guests }, camera);
  renderer.waitForGpu();
  const visible = new Set<string>();
  for (let y = 0; y < canvas.height; y += 8) for (let x = 0; x < canvas.width; x += 8) {
    const pick = renderer.pick(x, y);
    if (pick?.kind === "sprite" && pick.id.startsWith("guest-")) visible.add(pick.id);
  }
  return guests.filter((guest) => {
    if (!visible.has(guest.id)) return false;
    const frame = atlas.frames[`${guest.sprite}/${guest.view}/${guest.animation}/${guest.frame}`];
    if (!frame) throw new Error(`Missing guest frame for ${guest.id}`);
    const anchor = projectWorld(camera, canvas, { x: guest.x, y: guest.y, z: guest.height / 4 });
    const left = anchor.x - frame.anchor.x * camera.zoom;
    const top = anchor.y - frame.anchor.y * camera.zoom;
    return left >= 0 && top >= 0 && left + frame.width * camera.zoom <= canvas.width && top + frame.height * camera.zoom <= canvas.height;
  }).map((guest) => guest.id);
};
