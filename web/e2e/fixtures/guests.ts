import init, { WasmSimulation } from "../../src/wasm/theme_park_sim_wasm.js";
import { createWorldRenderer, loadAtlas } from "../../src/render";
import { buildPeople } from "../../src/render/people";
import { captureWorldSnapshot } from "../../src/world-snapshot";

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
  renderer.render({ ...captureWorldSnapshot(simulation).message, sprites: guests }, { centerX: 220, centerY: 256.5, zoom: 1.5, rotation: 0 });
  renderer.waitForGpu();
  const visible = new Set<string>();
  for (let y = 0; y < canvas.height; y += 8) for (let x = 0; x < canvas.width; x += 8) {
    const pick = renderer.pick(x, y);
    if (pick?.kind === "sprite" && pick.id.startsWith("guest-")) visible.add(pick.id);
  }
  return [...visible];
};
