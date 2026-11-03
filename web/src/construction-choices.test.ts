import { describe, expect, it } from "vitest";
import type { ConstructionPiece, PieceShape } from "./construction-protocol";
import { groupedPieces, pieceInGroup, steppedPiece, stepperOptions } from "./construction-choices";

function shape(id: string, turn: number, kind = "Track"): PieceShape {
  return { id, name: id, kind, turn, entrySlope: "Level", exitSlope: "Level", entryBank: "Level", exitBank: "Level", points: [] };
}
function piece(id: string, group: string, slope = "Level", bank = "Level", refusal = ""): ConstructionPiece {
  return { id, label: id, group, heading: "East", slope, bank, refusal };
}

const shapes = new Map([
  ["Straight", shape("Straight", 0)], ["Loop", shape("Loop", 0)], ["TurnSmallStandalone", shape("TurnSmallStandalone", 2)],
  ["TurnLargeStandalone", shape("TurnLargeStandalone", 2)], ["LevelToGentleUp", shape("LevelToGentleUp", 0)],
  ["LiftLevelToGentleUp", shape("LiftLevelToGentleUp", 0, "Lift")], ["LiftFlat", shape("LiftFlat", 0, "Lift")],
  ["LevelToGentleDown", shape("LevelToGentleDown", 0)], ["BankLeft", shape("BankLeft", 0)], ["BankRight", shape("BankRight", 0)],
]);
const pieces = [
  piece("Straight", "straight"), piece("LevelToGentleUp", "slope", "UpGentle", "Level", "The train would collide with terrain."),
  piece("LevelToGentleDown", "slope", "DownGentle"), piece("TurnSmallStandalone", "turn"), piece("TurnLargeStandalone", "turn"),
  piece("BankLeft", "bank", "Level", "Left"), piece("BankRight", "bank", "Level", "Right"), piece("Loop", "inversion"),
  piece("LiftFlat", "special"), piece("LiftLevelToGentleUp", "slope", "UpGentle"),
];

describe("construction steppers", () => {
  it("orders turns from left to right and slopes from down to up, and clamps at the ends", () => {
    expect(stepperOptions(pieces, shapes, "Straight", "turn")).toEqual({ values: ["Left 90°", "Ahead"], index: 1 });
    expect(stepperOptions(pieces, shapes, "Straight", "slope")).toEqual({ values: ["Gentle down", "Level", "Gentle up"], index: 1 });
    expect(stepperOptions(pieces, shapes, "Straight", "bank")).toEqual({ values: ["Left", "Level", "Right"], index: 1 });
    expect(steppedPiece(pieces, shapes, "Straight", "turn", -1)).toBe("TurnSmallStandalone");
    expect(steppedPiece(pieces, shapes, "Straight", "turn", 1)).toBeUndefined();
    expect(steppedPiece(pieces, shapes, "Straight", "bank", 1)).toBe("BankRight");
    expect(steppedPiece(pieces, shapes, "Straight", "bank", -1)).toBe("BankLeft");
    expect(steppedPiece(pieces, shapes, "Straight", "slope", -1)).toBe("LevelToGentleDown");
  });

  it("prefers placeable pieces of the same kind when several share the stepped value", () => {
    expect(steppedPiece(pieces, shapes, "Straight", "slope", 1)).toBe("LiftLevelToGentleUp");
    expect(steppedPiece(pieces, shapes, "LiftLevelToGentleUp", "slope", -1)).toBe("LiftFlat");
    expect(steppedPiece(pieces, shapes, "LevelToGentleUp", "slope", -1)).toBe("Straight");
  });

  it("cycles through the palette in group order and jumps to a group's first placeable piece", () => {
    const groups = groupedPieces(pieces);
    expect(groups.map(({ group }) => group)).toEqual(["straight", "turn", "slope", "bank", "inversion", "special"]);
    expect(groups.find(({ group }) => group === "slope")!.pieces.map(({ id }) => id)).toEqual(["LevelToGentleUp", "LevelToGentleDown", "LiftLevelToGentleUp"]);
    expect(steppedPiece(pieces, shapes, "Straight", "piece", 1)).toBe("TurnSmallStandalone");
    expect(steppedPiece(pieces, shapes, "Straight", "piece", -1)).toBe("LiftFlat");
    expect(pieceInGroup(pieces, "slope")).toBe("LevelToGentleDown");
    expect(pieceInGroup(pieces, "inversion")).toBe("Loop");
    expect(pieceInGroup(pieces, "unknown")).toBeUndefined();
  });
});
