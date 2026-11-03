import type { BlockReport } from "./construction-protocol";

export interface BlockPanelState {
  readonly busy: boolean;
  readonly canApplyBest: boolean;
  readonly statusText: string;
  readonly capacityText: string;
  readonly throughputText: string;
  readonly message: string;
}

const friendlyBlockMessages: Readonly<Record<string, string>> = {
  "A closed circuit is required to place block sections.": "Close the circuit before placing block sections.",
  "A closed circuit is required to edit block sections.": "Close the circuit before editing block sections.",
  "There is no automatic block placement search in progress.": "Start automatic placement before applying its best layout.",
  "Place automatic block sections before editing them.": "Place block sections automatically before editing them.",
  "Block section index is out of range.": "That block section is no longer available. Choose a section from the list.",
  "train length and car count must fit the circuit": "The completed circuit must be longer than the full train.",
  "block boundary is outside the circuit": "Choose a boundary on the coaster circuit.",
  "duplicate block boundary": "Choose a boundary that another section does not already use.",
  "boundary must end an eligible brake run long enough for the train": "Choose a listed level brake run that is long enough to hold the train.",
  "block brake runs cannot overlap": "Choose brake runs that do not overlap.",
  "block brakes require powered drive and braking": "This coaster needs powered drive and brakes to use block sections.",
  "train cannot safely reach and stop on the next block brake": "This layout cannot bring the train to a safe stop at the next brake.",
  "train stops before it fits on the block brake": "The brake run needs more room to stop the whole train.",
  "train stalled or rolled back between block brakes": "The train cannot reliably reach the next block brake.",
  "train did not reach the next block brake": "The train could not reach the next block brake.",
  "block placement physics work budget exhausted": "The block placement check reached its work limit. Try a simpler layout.",
  "a train recovered during block capacity validation": "The train could not complete this block layout safely.",
  "a train needed an emergency stop during block capacity validation": "This block layout would require an emergency stop.",
  "A train recovered during the throughput simulation.": "The train could not complete the throughput check safely.",
  "The throughput simulation did not observe enough sustained dispatches.": "The throughput check could not observe enough completed dispatches.",
};

/** Convert the worker's block report into concise status text for the construction panel. */
export function blockPanelState(report: BlockReport): BlockPanelState {
  let busy = false;
  let statusText: string;
  switch (report.placementStatus) {
    case "idle":
      statusText = "Place block sections automatically to see the train capacity.";
      break;
    case "searching":
      busy = true;
      statusText = "Finding block sections. The installed layout remains in use.";
      break;
    case "validating":
      busy = true;
      statusText = "Checking the block section change. The installed layout remains in use.";
      break;
    case "manual":
      statusText = "Manual block sections are in use.";
      break;
    case "complete":
      statusText = "Automatic placement is complete.";
      break;
    case "incomplete":
      statusText = "This is the best verified layout so far. Additional trains may be possible.";
      break;
    default:
      throw new Error(`Unknown block placement status ${report.placementStatus}`);
  }

  const capacityText = report.trainCapacity === 0
    ? ""
    : `Installed layout supports up to ${report.trainCapacity} ${report.trainCapacity === 1 ? "train" : "trains"}.`;

  let throughputText: string;
  switch (report.throughputStatus) {
    case "ready":
      if (report.theoreticalHourlyRiders === null || !Number.isFinite(report.theoreticalHourlyRiders)) {
        throw new Error("Ready block throughput report is missing a finite rider estimate");
      }
      throughputText = `Estimated throughput: ${report.theoreticalHourlyRiders.toLocaleString("en-US")} riders per hour.`;
      break;
    case "pending":
      throughputText = "Estimating hourly riders…";
      break;
    case "unavailable":
      throughputText = "";
      break;
    default:
      throw new Error(`Unknown block throughput status ${report.throughputStatus}`);
  }

  return {
    busy,
    canApplyBest: report.placementStatus === "searching",
    statusText,
    capacityText,
    throughputText,
    message: friendlyBlockMessages[report.reasons[0] ?? ""] ?? report.reasons[0] ?? "",
  };
}

/** Keep candidate distances exact and remove boundaries already occupied by installed sections. */
export function availableBlockBoundaries(candidates: Float64Array, report: BlockReport): number[] {
  const occupiedBoundaries = new Set(report.sections.map(({ boundaryMetres }) => boundaryMetres));
  const available: number[] = [];
  let previousBoundary = -Infinity;
  for (const boundaryMetres of candidates) {
    if (!Number.isFinite(boundaryMetres) || boundaryMetres <= previousBoundary) {
      throw new Error("Block boundary candidates must be finite, unique, and sorted");
    }
    previousBoundary = boundaryMetres;
    if (!occupiedBoundaries.has(boundaryMetres)) available.push(boundaryMetres);
  }
  return available;
}

/** Return the adjacent train count when it stays between one and verified capacity. */
export function steppedTrainCount(trainCount: number, trainCapacity: number, step: -1 | 1): number | undefined {
  if (!Number.isInteger(trainCount) || trainCount < 1 || !Number.isInteger(trainCapacity) || trainCapacity < trainCount) {
    throw new RangeError("Train count must be within the verified capacity");
  }
  const nextTrainCount = trainCount + step;
  return nextTrainCount < 1 || nextTrainCount > trainCapacity ? undefined : nextTrainCount;
}
