import { describe, expect, it } from "vitest";
import type { BlockReport } from "./construction-protocol";
import { availableBlockBoundaries, blockPanelState, steppedTrainCount } from "./block-panel-state";

function report(update: Partial<BlockReport> = {}): BlockReport {
  return {
    sections: [],
    trainCapacity: 0,
    capacitySearchComplete: null,
    theoreticalHourlyRiders: null,
    throughputStatus: "unavailable",
    placementStatus: "idle",
    placementSearchComplete: null,
    reasons: [],
    notes: [],
    suggestions: [],
    ...update,
  };
}

describe("block panel state", () => {
  it("explains how to get the first block layout and capacity report", () => {
    expect(blockPanelState(report())).toEqual({
      busy: false,
      canApplyBest: false,
      statusText: "Place block sections automatically to see the train capacity.",
      capacityText: "",
      throughputText: "",
      message: "",
    });
  });

  it("keeps the installed capacity visible while automatic placement is searching", () => {
    expect(blockPanelState(report({
      placementStatus: "searching",
      throughputStatus: "pending",
      trainCapacity: 2,
    }))).toEqual({
      busy: true,
      canApplyBest: true,
      statusText: "Finding block sections. The installed layout remains in use.",
      capacityText: "Installed layout supports up to 2 trains.",
      throughputText: "Estimating hourly riders…",
      message: "",
    });
  });

  it("shows validation feedback and the last installed layout while an edit is checked", () => {
    expect(blockPanelState(report({
      placementStatus: "validating",
      throughputStatus: "pending",
      trainCapacity: 1,
      reasons: ["duplicate block boundary"],
    }))).toEqual({
      busy: true,
      canApplyBest: false,
      statusText: "Checking the block section change. The installed layout remains in use.",
      capacityText: "Installed layout supports up to 1 train.",
      throughputText: "Estimating hourly riders…",
      message: "Choose a boundary that another section does not already use.",
    });
  });

  it("reports ready capacity and simulated hourly riders", () => {
    expect(blockPanelState(report({
      placementStatus: "complete",
      throughputStatus: "ready",
      trainCapacity: 3,
      theoreticalHourlyRiders: 1284.5,
    }))).toEqual({
      busy: false,
      canApplyBest: false,
      statusText: "Automatic placement is complete.",
      capacityText: "Installed layout supports up to 3 trains.",
      throughputText: "Estimated throughput: 1,284.5 riders per hour.",
      message: "",
    });
  });

  it("preserves unknown backend messages for future report reasons", () => {
    expect(blockPanelState(report({ reasons: ["A new simulation reason."] })).message).toBe("A new simulation reason.");
  });

  it("keeps exact candidate distances and removes occupied boundaries", () => {
    const installedBoundary = 12.1234567890123;
    const availableBoundary = 24.9876543210987;
    const blockReport = report({ sections: [{
      boundaryMetres: installedBoundary,
      brakeStartMetres: 3,
      brakePieceIndices: [2, 3],
      poweredHoldStartMetres: null,
      poweredHoldPieceIndices: [],
      editable: true,
      editableIndex: 0,
      reason: "Placed on a level brake run long enough to hold a train.",
      trackKind: "blockBrake",
    }] });
    expect(availableBlockBoundaries(new Float64Array([installedBoundary, availableBoundary]), blockReport)).toEqual([availableBoundary]);
  });
});

describe("train count stepper", () => {
  it("moves by one while staying inside the layout's supported capacity", () => {
    expect(steppedTrainCount(2, 3, -1)).toBe(1);
    expect(steppedTrainCount(2, 3, 1)).toBe(3);
  });

  it("does not step below one train or above the verified capacity", () => {
    expect(steppedTrainCount(1, 3, -1)).toBeUndefined();
    expect(steppedTrainCount(3, 3, 1)).toBeUndefined();
  });
});
