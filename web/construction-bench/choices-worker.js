import init, { WasmSimulation } from '../src/wasm/theme_park_sim_wasm.js';

try {
  await init();
  const simulation = new WasmSimulation(2026n, 12, 'demo');
  simulation.construction_start(50, 50, 160, 0);
  const pieces = [
    ...Array(72).fill('Straight'), 'TurnSmallStandalone', 'TurnSmallStandalone',
    ...Array(73).fill('Straight'), 'TurnSmallStandalone', 'TurnSmallStandalone',
  ];
  for (const [index, piece] of pieces.entries()) {
    simulation.construction_select(piece);
    simulation.construction_append();
    const track = JSON.parse(simulation.construction_track());
    if (track.length !== index + 2) throw new Error(`Piece ${index + 1} failed: ${simulation.construction_message()}`);
  }
  simulation.construction_close();
  if (!simulation.construction_can_test()) throw new Error('Circuit did not close');
  simulation.construction_choices();
  const start = performance.now();
  simulation.construction_delete(75);
  const deleteMs = performance.now() - start;
  const choiceStart = performance.now();
  const choices = simulation.construction_choices();
  const firstMs = performance.now() - start;
  const choiceMs = performance.now() - choiceStart;
  const repeatStart = performance.now();
  const repeated = simulation.construction_choices();
  const repeatMs = performance.now() - repeatStart;
  if (JSON.parse(simulation.construction_track()).length !== 149) throw new Error('Deletion failed');
  if (choices !== repeated || !choices) throw new Error('Choice refresh returned inconsistent results');
  self.postMessage({ firstMs, deleteMs, choiceMs, repeatMs, rows: JSON.parse(choices).length });
  simulation.free();
} catch (error) {
  self.postMessage({ error: String(error) });
}
