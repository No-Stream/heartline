import { describe, expect, it } from "vitest";
import type { ConstructionPiece, PieceShape } from "./construction-protocol";
import { groupedPieces, pieceDisplayName, pieceInGroup, steppedPiece, stepperOptions } from "./construction-choices";

function shape(id: string, turn: number, kind = "Track"): PieceShape {
  return { id, name: id, kind, turn, entrySlope: "Level", exitSlope: "Level", entryBank: "Level", exitBank: "Level", points: [] };
}
function piece(id: string, group: string, slope = "Level", bank = "Level", refusal = ""): ConstructionPiece {
  return { id, label: id, group, heading: "East", slope, bank, refusal, wouldTunnel: false };
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
    expect(stepperOptions(pieces, shapes, "Straight", "slope")).toEqual({ values: ["Gentle down 27°", "Level", "Gentle up 27°"], index: 1 });
    expect(stepperOptions(pieces, shapes, "Straight", "bank")).toEqual({ values: ["Left", "Level", "Right"], index: 1 });
    expect(steppedPiece(pieces, shapes, "Straight", "turn", -1)).toBe("TurnSmallStandalone");
    expect(steppedPiece(pieces, shapes, "Straight", "turn", 1)).toBeUndefined();
    expect(steppedPiece(pieces, shapes, "Straight", "bank", 1)).toBe("BankRight");
    expect(steppedPiece(pieces, shapes, "Straight", "bank", -1)).toBe("BankLeft");
    expect(steppedPiece(pieces, shapes, "Straight", "slope", -1)).toBe("LevelToGentleDown");
  });

  it("offers inverted exits after upright bank states", () => {
    const invertedPieces = [piece("Straight", "straight"), piece("HalfRoll", "inversion", "Level", "Inverted")];
    const invertedShapes = new Map(invertedPieces.map(({ id }) => [id, shape(id, 0)]));
    expect(stepperOptions(invertedPieces, invertedShapes, "Straight", "bank")).toEqual({ values: ["Level", "Inverted"], index: 0 });
    expect(steppedPiece(invertedPieces, invertedShapes, "Straight", "bank", 1)).toBe("HalfRoll");
  });

  it("orders 60-degree slope states outside the 45-degree slopes and labels every slope with its angle", () => {
    const sixtyDegreePieces = [
      piece("DownSixty", "slope", "DownSixty"),
      piece("DownSteep", "slope", "DownSteep"),
      piece("UpSteep", "slope", "UpSteep"),
      piece("UpSixty", "slope", "UpSixty"),
    ];
    const sixtyDegreeShapes = new Map(sixtyDegreePieces.map(({ id }) => [id, shape(id, 0)]));

    expect(stepperOptions(sixtyDegreePieces, sixtyDegreeShapes, "DownSteep", "slope")).toEqual({
      values: ["Very steep down 60°", "Steep down 45°", "Steep up 45°", "Very steep up 60°"],
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

  it("adds climbing and descending to turn names", () => {
    const climbingTurn = { ...piece("TurnSmallStandalone", "turn", "UpGentle"), label: "Small left turn" };
    const descendingTurn = { ...piece("TurnSmallStandalone", "turn", "DownGentle"), label: "Small left turn" };
    const levelTurn = { ...piece("TurnSmallStandalone", "turn"), label: "Small left turn" };
    const climbingSlope = { ...piece("GentleUp", "slope", "UpGentle"), label: "Gentle climb" };

    expect(pieceDisplayName(climbingTurn)).toBe("Climbing small left turn");
    expect(pieceDisplayName(descendingTurn)).toBe("Descending small left turn");
    expect(pieceDisplayName(levelTurn)).toBe("Small left turn");
    expect(pieceDisplayName(climbingSlope)).toBe("Gentle climb");
  });

  it("uses plain names for complete turns and start or end phrases for turn segments", () => {
    const turn = (label: string, slope: string) => ({ ...piece("TurnSmallEntry", "turn", slope), label });

    expect(pieceDisplayName(turn("Begin small left turn", "UpGentle"))).toBe("Start of a climbing small left turn");
    expect(pieceDisplayName(turn("End large right-banked right turn", "DownGentle"))).toBe("End of a descending large right-banked turn");
    expect(pieceDisplayName(turn("Standalone small left turn", "UpGentle"))).toBe("Climbing small left turn");
    expect(pieceDisplayName(turn("Standalone large right turn", "Level"))).toBe("Large right turn");
    expect(pieceDisplayName(turn("Begin small left-banked left turn", "Level"))).toBe("Start of a small left-banked turn");
    expect(pieceDisplayName(turn("End small right-banked right turn", "Level"))).toBe("End of a small right-banked turn");
  });

  it("names the three-piece steep curves by direction and place in the sequence", () => {
    const verticalCurvePieces = [
      piece("VerticalUpEntry", "slope", "UpSteep"),
      piece("VerticalUpContinue", "slope", "UpSteep"),
      piece("VerticalUpExit", "slope", "Level"),
      piece("VerticalDownEntry", "slope", "DownSteep"),
      piece("VerticalDownContinue", "slope", "DownSteep"),
      piece("VerticalDownExit", "slope", "Level"),
    ];
    const labels = [
      "Begin upward curve",
      "Continue upward curve",
      "End upward curve",
      "Begin downward curve",
      "Continue downward curve",
      "End downward curve",
    ];

    expect(verticalCurvePieces.map((verticalPiece, index) => pieceDisplayName({
      ...verticalPiece,
      label: labels[index],
    }))).toEqual([
      "Start of steep upward curve",
      "Middle of steep upward curve",
      "End of steep upward curve",
      "Start of steep downward curve",
      "Middle of steep downward curve",
      "End of steep downward curve",
    ]);
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

  it("shows banked straights with straight pieces and roll-outs with bank transitions", () => {
    const bankedContinuations = [
      { ...piece("BankedStraightLeft", "straight", "Level", "Left"), label: "Left-banked straight" },
      { ...piece("BankedStraightRight", "straight", "Level", "Right"), label: "Right-banked straight" },
      { ...piece("RolloutLeft", "bank", "Level", "Level"), label: "Roll out from left bank" },
      { ...piece("RolloutRight", "bank", "Level", "Level"), label: "Roll out from right bank" },
    ];

    const groups = groupedPieces(bankedContinuations);

    expect(groups.map(({ group, pieces: members }) => [group, members.map(({ id }) => id)])).toEqual([
      ["straight", ["BankedStraightLeft", "BankedStraightRight"]],
      ["bank", ["RolloutLeft", "RolloutRight"]],
    ]);
    expect(bankedContinuations.map(pieceDisplayName)).toEqual([
      "Left-banked straight",
      "Right-banked straight",
      "Roll out from left bank",
      "Roll out from right bank",
    ]);
  });
});
