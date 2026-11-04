import { createTestingPanel, type TestingReport } from "../../src/coaster-testing";
import { createConstructionPanel } from "../../src/construction";
import type { ConstructionState, PieceShape } from "../../src/construction-protocol";
import type { InputAction } from "../../src/input/actions";
import { bindKeyboard } from "../../src/input/bindings";

const actions: InputAction[] = [];
const container = document.querySelector<HTMLElement>("section")!;
/** Stands in for main.ts and the worker: piece steps resolve in the panel, and a selection comes back as the new state. */
function dispatch(action: InputAction): void {
  actions.push(action);
  if (action.type === "piece-step" || action.type === "piece-group") panel.choose(action);
  if (action.type === "construction-select") setConstructionState({ selected: action.piece });
}
const panel = createConstructionPanel(container, dispatch);
bindKeyboard(container, dispatch, () => ({ x: 400, y: 250 }));

const line = (length: number, turn: number): PieceShape["points"] => Array.from({ length: 33 }, (_, index) => {
  const fraction = index / 32;
  if (turn === 0) return [length * fraction, 0, 0, 0, 0, 1] as const;
  const angle = fraction * Math.PI / 2;
  return [length * Math.sin(angle), length * (1 - Math.cos(angle)), 0, 0, 0, 1] as const;
});
const shape = (id: string, name: string, kind: string, turn: number, exitSlope = "Level"): PieceShape => ({
  id, name, kind, turn: turn * 2, entrySlope: "Level", exitSlope, entryBank: "Level", exitBank: "Level", points: line(16, turn),
});
panel.setShapes([
  shape("Station", "Station", "Station", 0), shape("Straight", "Straight", "Track", 0),
  shape("TurnSmallStandalone", "Standalone small left turn", "Track", 1), shape("LevelToGentleUp", "Begin gentle climb", "Track", 0, "UpGentle"),
]);
const state: ConstructionState = {
  type: "construction-state", active: true, chain: false,
  pieces: [
    { id: "Straight", label: "Straight", group: "straight", heading: "East", slope: "Level", bank: "Level", refusal: "" },
    { id: "TurnSmallStandalone", label: "Standalone small left turn", group: "turn", heading: "North", slope: "Level", bank: "Level", refusal: "" },
    { id: "LevelToGentleUp", label: "Begin gentle climb", group: "slope", heading: "East", slope: "UpGentle", bank: "Level", refusal: "The train would collide with terrain." },
  ], selected: "Straight",
  track: [
    { id: 4, label: "Station", name: "Station", group: "special", slope: "Level" },
    { id: 9, label: "Straight", name: "Straight", group: "straight", slope: "Level" },
  ], canAppend: true, canClose: false, finishPreview: false, canFinish: false, finishReason: "Build more track to reach a connection.", canTest: false, isOpen: false, coasterPrice: 6, coasterIncome: 0n, stationPositions: [227, 251, 12, 227, 253, 12], message: "Ready to place",
  blockReport: { sections: [], trainCapacity: 0, capacitySearchComplete: null, theoreticalHourlyRiders: null, throughputStatus: "unavailable", placementStatus: "idle", placementSearchComplete: null, reasons: [], notes: [], suggestions: [] },
  trainCount: 1, placedRails: new Float32Array(), railDistances: new Float32Array(), blockCandidates: new Float64Array(), placedTies: new Float32Array(), placedSupports: new Float32Array(),
  previewRails: new Float32Array(), previewTies: new Float32Array(), previewSupports: new Float32Array(),
};
let current = state;
function setConstructionState(update: Partial<ConstructionState>): void { current = { ...current, ...update }; panel.update(current); }
panel.update(state);
Object.assign(window, { constructionActions: actions, setConstructionState });

const testingPanel = createTestingPanel(document.querySelector<HTMLElement>("[data-testing]")!,
  { steel: "#e04730", train: "#1f85ad", wood: "#805030", ink: "#211519", lineWeight: 1 }, () => {});
Object.assign(window, { setTestingReport: (report: TestingReport | null) => testingPanel.update(report) });
