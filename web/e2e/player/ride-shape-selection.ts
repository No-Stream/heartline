import {
  defaultInversionSelectionId,
  defaultShapeSelectionId,
  shapeSelectionId,
  type ShapedElementSelection,
  type ShapedLoopTopFeel,
} from "../../src/construction-shaped-elements.ts";
import type { RideShapeIntent } from "./rides.ts";

function topFeelFromRideIntent(feel: "Floaty" | "Neutral" | "PressedIn"): ShapedLoopTopFeel {
  switch (feel) {
    case "Floaty": return "floaty";
    case "Neutral": return "neutral";
    case "PressedIn": return "pressedIn";
  }
}

/** How the visible builder controls express a ride file's shaped intent; shared by the browser driver and the fast agreement check. */
export function selectionFromRideIntent(intent: RideShapeIntent): ShapedElementSelection {
  if ("hill" in intent) {
    const hill = intent.hill;
    let feel: "ejector" | "floater" | "strongFloater";
    switch (hill.feel) {
      case "Ejector": feel = "ejector"; break;
      case "Floater": feel = "floater"; break;
      case "StrongFloater": feel = "strongFloater"; break;
    }
    const size = hill.size === "Auto"
      ? { kind: "auto" as const }
      : { kind: "height" as const, heightMetres: hill.size.Height };
    return { kind: "hill", feel, strengthG: hill.strength_g, size };
  }
  if ("turn" in intent) {
    const turn = intent.turn;
    const size = turn.size === "Auto"
      ? { kind: "auto" as const }
      : { kind: "radius" as const, radiusMetres: turn.size.Radius };
    return {
      kind: "turn",
      hand: turn.hand === "Left" ? "left" : "right",
      feel: "fullyBanked",
      size,
    };
  }
  if ("loop" in intent) {
    return {
      kind: "loop",
      hand: intent.loop.hand === "Left" ? "left" : "right",
      peakG: intent.loop.peak_g,
      topFeel: topFeelFromRideIntent(intent.loop.top_feel),
    };
  }
  if ("zero_g_roll" in intent) {
    return {
      kind: "zeroGRoll",
      hand: intent.zero_g_roll.hand === "Left" ? "left" : "right",
      peakG: intent.zero_g_roll.peak_g,
    };
  }
  return {
    kind: "ravenTurn",
    hand: intent.raven_turn.hand === "Left" ? "left" : "right",
    peakG: intent.raven_turn.peak_g,
    topFeel: topFeelFromRideIntent(intent.raven_turn.top_feel),
  };
}

export function shapeBaseSelectionId(selection: ShapedElementSelection): string {
  if (selection.kind === "hill") {
    return defaultShapeSelectionId(selection.feel);
  }
  if (selection.kind === "turn") {
    return shapeSelectionId({ ...selection, size: { kind: "auto" } });
  }
  return defaultInversionSelectionId(selection.kind, selection.hand);
}

export type ClassicHillAttribute = "feel" | "twist" | "height" | "span";

export const hillFeelOrder = ["Ejector", "Strong floater", "Floater", "Speed hill", "Zero-g roll", "Stall"];
export const hillTwistOrder = ["Outer bank left", "Upright", "Outer bank right", "Roll left", "Roll right"];
/** The order in which the driver steps the hill inspector's values. */
export const hillAttributeOrder: readonly ClassicHillAttribute[] = ["feel", "twist", "height", "span"];

/** The hill inspector values (metric labels) that select a classic hill, roll or stall, or null for other pieces. */
export function classicHillTargets(piece: string): Readonly<Record<ClassicHillAttribute, string>> | null {
  const hill = /^(WoodenEjector|Ejector|Floater|Family|Speed)Hill(\d+)x(\d+)(OuterLeft|OuterRight)?$/.exec(piece);
  const roll = /^ZeroGRoll(Fast|Compact)(Left|Right)$/.exec(piece);
  const stall = /^Stall(Left|Right)$/.exec(piece);
  if (!hill && !roll && !stall) return null;
  const feel = stall ? "Stall" : roll ? "Zero-g roll" : piece === "WoodenEjectorHill8x64" ? "Strong floater"
    : hill![1].includes("Ejector") ? "Ejector" : hill![1] === "Floater" || hill![1] === "Family" ? "Floater" : "Speed hill";
  const twist = stall ? `Roll ${stall[1].toLowerCase()}` : roll ? `Roll ${roll[2].toLowerCase()}`
    : hill?.[4] === "OuterLeft" ? "Outer bank left" : hill?.[4] === "OuterRight" ? "Outer bank right" : "Upright";
  const height = `${stall ? 13 : roll ? (roll[1] === "Fast" ? 8 : 10) : hill![2]} m`;
  const span = `${stall ? 104 : roll ? (roll[1] === "Fast" ? 86 : 80) : hill![3]} m`;
  return { feel, twist, height, span };
}
