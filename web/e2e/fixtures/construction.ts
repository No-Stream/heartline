import { createTestingPanel, type TestingReport } from "../../src/coaster-testing";
import { createConstructionPanel } from "../../src/construction";
import type { ConstructionState } from "../../src/construction-protocol";
import type { InputAction } from "../../src/input/actions";

const actions: InputAction[] = [];
const panel = createConstructionPanel(document.querySelector("section")!, (action) => actions.push(action));
const state: ConstructionState = {
  type: "construction-state", active: true,
  pieces: [{ id: "straight", label: "Straight", group: "Track", heading: "East", slope: "Level", bank: "Level" }, { id: "turn", label: "Turn", group: "Track", heading: "South", slope: "Level", bank: "Level" }], selected: "straight",
  track: [{ id: 4, label: "Station" }, { id: 9, label: "Straight" }], canAppend: true, canClose: false, canTest: false, message: "Ready to place",
  placedRails: new Float32Array(), placedTies: new Float32Array(), placedSupports: new Float32Array(),
  previewRails: new Float32Array(), previewTies: new Float32Array(), previewSupports: new Float32Array(),
};
panel.update(state);
Object.assign(window, { constructionActions: actions, setConstructionState: (update: Partial<ConstructionState>) => panel.update({ ...state, ...update }) });

const testingPanel = createTestingPanel(document.querySelector<HTMLElement>("[data-testing]")!,
  { steel: "#e04730", train: "#1f85ad", wood: "#805030", ink: "#211519", lineWeight: 1 }, () => {});
Object.assign(window, { setTestingReport: (report: TestingReport | null) => testingPanel.update(report) });
