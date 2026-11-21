import type { CandidateForcePreview } from "./candidate-force-readout";
import type {
  ConstructionAction,
  ConstructionElement,
  ConstructionForcePreview,
  ConstructionHillOption,
  ConstructionPiece,
  ConstructionShapedElement,
  ConstructionSpeedTurn,
} from "./construction-protocol";
import type { ConstructionSpeedMatch } from "./construction-speed-match";
import type { BuilderSessionBridge, WasmSimulation } from "./wasm/theme_park_sim_wasm.js";

/**
 * The builder commands and estimate reads the simulation worker performs, kept free of worker globals so the
 * fast ride-agreement check (web/e2e/rides/fast-agreement.ts) runs exactly this code against the same WASM build.
 */

export type BuilderWorkerEstimates = {
  readonly constructionForcePreview: ConstructionForcePreview | null;
  readonly pieces: ConstructionPiece[];
  readonly elements: ConstructionElement[];
  readonly hillOptions: ConstructionHillOption[];
  readonly speedTurns: ConstructionSpeedTurn[];
  readonly selectedShape: ConstructionShapedElement | null;
  readonly candidateForcePreview: CandidateForcePreview | null;
  readonly speedMatch: ConstructionSpeedMatch | null;
};

export function readBuilderEstimates(
  simulation: WasmSimulation,
  builderSession: BuilderSessionBridge,
  complete: boolean,
): BuilderWorkerEstimates {
  return JSON.parse(builderSession.estimates(simulation, complete)) as BuilderWorkerEstimates;
}

/** Applies one builder command; returns false for commands that only change main-thread state. */
export function applyConstructionAction(
  simulation: WasmSimulation,
  builderSession: BuilderSessionBridge,
  action: ConstructionAction | { readonly type: "construction-delete-mode" },
  scheduleFinishSearch: () => void,
): boolean {
  if (action.type !== "construction-select") builderSession.invalidate();
  switch (action.type) {
    case "construction-start": builderSession.start(simulation, action.x, action.y, action.z, action.heading); break;
    case "construction-select": builderSession.select(simulation, action.piece, action.exact ?? false); break;
    case "construction-set-chain": simulation.construction_set_chain(action.enabled); break;
    case "construction-set-launch-intensity": simulation.construction_set_launch_intensity(action.id, action.targetSpeed); break;
    case "construction-append": simulation.construction_append(); break;
    case "construction-append-resolved": simulation.construction_append_resolved(action.intentJson, action.specJson); break;
    case "construction-undo": simulation.construction_undo(); break;
    case "construction-delete": simulation.construction_delete(action.id); break;
    case "construction-close": simulation.construction_close(); break;
    case "construction-finish-preview": simulation.construction_finish_preview(); break;
    case "construction-finish-confirm": simulation.construction_finish_confirm(); scheduleFinishSearch(); break;
    case "construction-finish-cycle": simulation.construction_finish_cycle(action.step); break;
    case "construction-finish-cancel": simulation.construction_finish_cancel(); break;
    case "construction-set-open": simulation.construction_set_open(action.open); break;
    case "construction-set-coaster-price": simulation.construction_set_coaster_price(action.price); break;
    case "construction-set-colors": simulation.construction_set_colors(action.colors.track, action.colors.supports, action.colors.trains); break;
    case "construction-test": simulation.construction_test(); break;
    case "construction-auto-blocks": simulation.construction_auto_blocks(); break;
    case "construction-apply-best-blocks": simulation.construction_apply_best_block_sections(); break;
    case "construction-set-train-count": simulation.construction_set_train_count(action.count); break;
    case "construction-set-cars-per-train": simulation.construction_set_cars_per_train(action.cars); break;
    case "construction-move-block": simulation.construction_move_block_section(action.index, action.boundaryMetres); break;
    case "construction-remove-block": simulation.construction_remove_block_section(action.index); break;
    case "construction-add-block": simulation.construction_add_block_section(action.boundaryMetres); break;
    case "construction-set-block-sections": simulation.construction_install_block_layout(
      JSON.stringify(action.blockSections),
      action.trainCapacity,
      action.capacitySearchComplete,
      action.placementSearchComplete,
    ); break;
    case "construction-delete-mode": return false;
    default: {
      const unhandled: never = action;
      throw new Error(`Unhandled construction action ${JSON.stringify(unhandled)}`);
    }
  }
  return true;
}

/** Track edits send geometry first and estimates after, as one incomplete then one complete construction state. */
export function sendsIncompleteConstructionState(action: ConstructionAction, simulation: WasmSimulation): boolean {
  return ["construction-start", "construction-append", "construction-append-resolved", "construction-undo", "construction-delete"].includes(action.type)
    && simulation.construction_active() && !simulation.construction_selected().startsWith("element:");
}
