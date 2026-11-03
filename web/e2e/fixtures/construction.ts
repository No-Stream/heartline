import { createConstructionPanel } from "../../src/construction";
import type { ConstructionAction, ConstructionState } from "../../src/construction-protocol";

const actions: ConstructionAction[] = [];
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
