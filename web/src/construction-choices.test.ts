import { describe, expect, it } from "vitest";
import type { ConstructionHillOption, ConstructionPiece, PieceShape } from "./construction-protocol";
import { speedMatchedSelection } from "./construction-speed-match";
import { steppedPieceSelection, groupedPieces, hillStepperOptions, hillToOpen, pieceDisplayName, pieceInGroup, steppedHillSelection, steppedPiece, stepperOptions, type HillAttribute } from "./construction-choices";

function shape(id: string, turn: number, kind = "Track"): PieceShape {
  return { id, name: id, kind, turn, entrySlope: "Level", exitSlope: "Level", entryBank: "Level", exitBank: "Level", points: [] };
}
function piece(id: string, group: string, slope = "Level", bank = "Level", refusal = ""): ConstructionPiece {
  return { id, label: id, group, heading: "East", slope, bank, refusal, wouldTunnel: false, heartlined: null, hillEstimate: null };
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

it("cycles past speed-matched variants without redirecting to a previous size", () => {
  const palette = [piece("Loop", "inversion"), piece("HeartlineRoll", "inversion"), piece("LargeLoop", "inversion"), piece("LaunchShort", "special")];
  let selected = "Loop";
  for (let index = 0; index < 3; index++) {
    const action = steppedPieceSelection(palette, shapes, selected, "piece", 1)!;
    expect(action.exact).toBe(true);
    selected = speedMatchedSelection(action.piece, { entrySpeed: 10, recommendedId: "Loop", fits: true, variants: [] }, action.exact);
  }
  expect(selected).toBe("LaunchShort");
});

describe("airtime hill steppers", () => {
  const hill = (feel: string, height: number, span: number, twist = "none"): ConstructionHillOption => ({
    id: `${feel}${height}x${span}${twist}`, feel, height, span, twist, ratedSpeed: { minimumEntry: 10, maximumEntry: 20 },
  });
  const hybrid = [
    hill("ejector", 8, 48), hill("ejector", 12, 64), hill("ejector", 8, 64), hill("ejector", 12, 80), hill("ejector", 16, 96),
    hill("floater", 4, 48), hill("floater", 8, 64), hill("floater", 12, 80), hill("speed", 4, 64), hill("speed", 6, 80),
    hill("ejector", 8, 64, "outerLeft"), hill("ejector", 8, 64, "outerRight"), hill("ejector", 12, 64, "outerLeft"), hill("ejector", 12, 64, "outerRight"),
    hill("zeroGRoll", 8, 86, "left"), hill("zeroGRoll", 8, 86, "right"), hill("stall", 13, 104, "left"), hill("stall", 13, 104, "right"),
  ];
  const step = (selected: string, attribute: HillAttribute, direction: -1 | 1) => steppedHillSelection(hybrid, selected, attribute, direction);

  it("lists the model's feels, the feel's twists and the family's sizes in stepper order", () => {
    expect(hillStepperOptions(hybrid, "ejector12x64none", "feel", "metric")).toEqual({ values: ["Ejector", "Floater", "Speed hill", "Zero-g roll", "Stall"], index: 0 });
    expect(hillStepperOptions(hybrid, "ejector12x64none", "twist", "metric")).toEqual({ values: ["Outer bank left", "Upright", "Outer bank right"], index: 1 });
    expect(hillStepperOptions(hybrid, "ejector12x64none", "height", "metric")).toEqual({ values: ["8 m", "12 m", "16 m"], index: 1 });
    expect(hillStepperOptions(hybrid, "ejector12x64none", "span", "imperial")).toEqual({ values: ["157 ft", "210 ft", "262 ft", "315 ft"], index: 1 });
    expect(hillStepperOptions(hybrid, "speed4x64none", "twist", "metric")).toEqual({ values: ["Upright"], index: 0 });
    expect(hillStepperOptions(hybrid, "stall13x104right", "twist", "metric")).toEqual({ values: ["Roll left", "Roll right"], index: 1 });
    expect(hillStepperOptions(hybrid, "Straight", "feel", "metric")).toBeUndefined();
  });

  it("steps height and length exactly, keeping the other dimension as close as the grid allows", () => {
    expect(step("ejector12x64none", "height", 1)).toEqual({ type: "construction-select", piece: "ejector16x96none", exact: true });
    expect(step("ejector12x80none", "height", -1)).toEqual({ type: "construction-select", piece: "ejector8x64none", exact: true });
    expect(step("ejector8x48none", "span", 1)).toEqual({ type: "construction-select", piece: "ejector8x64none", exact: true });
    expect(step("ejector8x64none", "span", 1)).toEqual({ type: "construction-select", piece: "ejector12x80none", exact: true });
    expect(step("ejector16x96none", "height", 1)).toBeUndefined();
    expect(step("ejector8x48none", "span", -1)).toBeUndefined();
  });

  it("lets the worker size a new feel or twist for the entry speed, starting from the most similar hill", () => {
    expect(step("ejector12x64outerLeft", "feel", 1)).toEqual({ type: "construction-select", piece: "floater12x80none", exact: false });
    expect(step("speed6x80none", "feel", 1)).toEqual({ type: "construction-select", piece: "zeroGRoll8x86left", exact: false });
    expect(step("zeroGRoll8x86right", "feel", 1)).toEqual({ type: "construction-select", piece: "stall13x104right", exact: false });
    expect(step("ejector12x64none", "twist", -1)).toEqual({ type: "construction-select", piece: "ejector12x64outerLeft", exact: false });
    expect(step("ejector12x64outerRight", "twist", 1)).toBeUndefined();
  });

  it("opens on the current hill or the most similar hill of the preferred feel", () => {
    const pieces = [piece("Straight", "straight"), piece("floater8x64none", "special")];
    expect(hillToOpen(hybrid, pieces, "ejector")).toBe("ejector8x64none");
    expect(hillToOpen(hybrid, pieces, null)).toBe("floater8x64none");
    expect(hillToOpen(hybrid.filter(({ feel }) => feel !== "ejector"), pieces, "ejector")).toBe("floater8x64none");
  });
});
