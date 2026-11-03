import { describe, expect, it } from "vitest";
import type { ConstructionPiece, PieceShape } from "./construction-protocol";
import { groupedPieces, pieceDisplayName, pieceInGroup, steppedPiece, stepperOptions } from "./construction-choices";

function shape(id: string, turn: number, kind = "Track"): PieceShape {
  return { id, name: id, kind, turn, entrySlope: "Level", exitSlope: "Level", entryBank: "Level", exitBank: "Level", points: [] };
}
function piece(id: string, group: string, slope = "Level", bank = "Level", refusal = ""): ConstructionPiece {
  return { id, label: id, group, heading: "East", slope, bank, refusal };
}

const shapes = new Map([
  ["Straight", shape("Straight", 0)], ["Loop", shape("Loop", 0)], ["TurnSmallStandalone", shape("TurnSmallStandalone", 2)],
  ["TurnLargeStandalone", shape("TurnLargeStandalone", 2)], ["LevelToGentleUp", shape("LevelToGentleUp", 0)],
  ["SteepToGentleUp", shape("SteepToGentleUp", 0)],
  ["LevelToGentleDown", shape("LevelToGentleDown", 0)], ["BankLeft", shape("BankLeft", 0)], ["BankRight", shape("BankRight", 0)],
  ["ClimbingTurnLeft", shape("ClimbingTurnLeft", 2)], ["ClimbingTurnRight", shape("ClimbingTurnRight", -2)],
  ["GentleUp", shape("GentleUp", 0)],
]);
const pieces = [
  piece("Straight", "straight"), piece("LevelToGentleUp", "slope", "UpGentle", "Level", "The train would collide with terrain."),
  piece("LevelToGentleDown", "slope", "DownGentle"), piece("TurnSmallStandalone", "turn"), piece("TurnLargeStandalone", "turn"),
  piece("BankLeft", "bank", "Level", "Left"), piece("BankRight", "bank", "Level", "Right"), piece("Loop", "inversion"),
  piece("SteepToGentleUp", "slope", "UpGentle"),
];

describe("construction steppers", () => {
  it("keeps each model's available slope transitions in the Slopes group", () => {
    const modelTransitions = [
      {
        model: "Timber Tumbler",
        ids: [
          ["GentleUpToLevel", "Level"], ["SteepToGentleUp", "UpGentle"],
          ["GentleDownToLevel", "Level"], ["SteepToGentleDown", "DownGentle"],
          ["VerticalUpEntry", "UpSteep"], ["VerticalUpContinue", "UpSteep"], ["VerticalUpExit", "Level"],
          ["VerticalDownEntry", "DownSteep"], ["VerticalDownContinue", "DownSteep"], ["VerticalDownExit", "Level"],
        ],
      },
      {
        model: "Velocity Viper",
        ids: [
          ["GentleUpToLevel", "Level"], ["SteepToGentleUp", "UpGentle"],
          ["GentleDownToLevel", "Level"], ["SteepToGentleDown", "DownGentle"],
          ["VerticalUpEntry", "UpSteep"], ["VerticalUpContinue", "UpSteep"], ["VerticalUpExit", "Level"],
          ["VerticalDownEntry", "DownSteep"], ["VerticalDownContinue", "DownSteep"], ["VerticalDownExit", "Level"],
        ],
      },
      {
        model: "Briar Blitz",
        ids: [
          ["GentleUpToLevel", "Level"], ["SteepToGentleUp", "UpGentle"],
          ["GentleDownToLevel", "Level"], ["SteepToGentleDown", "DownGentle"],
          ["VerticalUpEntry", "UpSteep"], ["VerticalUpContinue", "UpSteep"], ["VerticalUpExit", "Level"],
          ["VerticalDownEntry", "DownSteep"], ["VerticalDownContinue", "DownSteep"], ["VerticalDownExit", "Level"],
        ],
      },
    ] as const;

    for (const { model, ids } of modelTransitions) {
      const transitions = ids.map(([id, slope]) => piece(id, "slope", slope));
      const groups = groupedPieces([piece("Straight", "straight"), ...transitions]);
      const slopeGroup = groups.find(({ group }) => group === "slope");

      expect(slopeGroup?.pieces.map(({ id }) => id), model).toEqual(ids.map(([id]) => id));
    }
  });

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

  it("orders and labels 60-degree slope states outside the 45-degree slopes", () => {
    const sixtyDegreePieces = [
      piece("DownSixty", "slope", "DownSixty"),
      piece("DownSteep", "slope", "DownSteep"),
      piece("UpSteep", "slope", "UpSteep"),
      piece("UpSixty", "slope", "UpSixty"),
    ];
    const sixtyDegreeShapes = new Map(sixtyDegreePieces.map(({ id }) => [id, shape(id, 0)]));

    expect(stepperOptions(sixtyDegreePieces, sixtyDegreeShapes, "DownSteep", "slope")).toEqual({
      values: ["60° down", "Steep down", "Steep up", "60° up"],
      index: 1,
    });
  });

  it("offers heading changes among pieces with the same climbing slope", () => {
    const climbingPieces = [
      piece("ClimbingTurnRight", "turn", "UpGentle"),
      piece("GentleUp", "straight", "UpGentle"),
      piece("ClimbingTurnLeft", "turn", "UpGentle"),
    ];

    expect(stepperOptions(climbingPieces, shapes, "ClimbingTurnLeft", "turn")).toEqual({
      values: ["Left 90°", "Ahead", "Right 90°"],
      index: 0,
    });
    expect(steppedPiece(climbingPieces, shapes, "ClimbingTurnLeft", "turn", 1)).toBe("GentleUp");
  });

  it("adds climbing and descending prefixes to turn names", () => {
    const climbingTurn = { ...piece("TurnSmallStandalone", "turn", "UpGentle"), label: "Small left turn" };
    const descendingTurn = { ...piece("TurnSmallStandalone", "turn", "DownGentle"), label: "Small left turn" };
    const levelTurn = { ...piece("TurnSmallStandalone", "turn"), label: "Small left turn" };
    const climbingSlope = { ...piece("GentleUp", "slope", "UpGentle"), label: "Gentle climb" };

    expect(pieceDisplayName(climbingTurn)).toBe("Climbing small left turn");
    expect(pieceDisplayName(descendingTurn)).toBe("Descending small left turn");
    expect(pieceDisplayName(levelTurn)).toBe("Small left turn");
    expect(pieceDisplayName(climbingSlope)).toBe("Gentle climb");
  });

  it("prefers placeable pieces when several share the stepped value", () => {
    expect(steppedPiece(pieces, shapes, "Straight", "slope", 1)).toBe("SteepToGentleUp");
    expect(steppedPiece(pieces, shapes, "SteepToGentleUp", "slope", -1)).toBe("Straight");
    expect(steppedPiece(pieces, shapes, "LevelToGentleUp", "slope", -1)).toBe("Straight");
  });

  it("cycles through the palette in group order and jumps to a group's first placeable piece", () => {
    const groups = groupedPieces(pieces);
    expect(groups.map(({ group }) => group)).toEqual(["straight", "turn", "slope", "bank", "inversion"]);
    expect(groups.find(({ group }) => group === "slope")!.pieces.map(({ id }) => id)).toEqual(["LevelToGentleUp", "LevelToGentleDown", "SteepToGentleUp"]);
    expect(steppedPiece(pieces, shapes, "Straight", "piece", 1)).toBe("TurnSmallStandalone");
    expect(steppedPiece(pieces, shapes, "Straight", "piece", -1)).toBe("Loop");
    expect(pieceInGroup(pieces, "slope")).toBe("LevelToGentleDown");
    expect(pieceInGroup(pieces, "inversion")).toBe("Loop");
    expect(pieceInGroup(pieces, "unknown")).toBeUndefined();
  });
});
