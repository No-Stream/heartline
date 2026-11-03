import type { GuestActivity, GuestMemory, GuestThought } from "./protocol";

const ACTIVITY_LABELS: Readonly<Record<string, string>> = {
  wandering: "Looking around",
  walking: "Taking a stroll",
  queuing: "Waiting in line",
  riding: "On a ride",
  eating: "Grabbing a bite",
  drinking: "Getting a drink",
  restroom: "Visiting the restroom",
  resting: "Taking a breather",
  waiting: "Waiting",
};

const PROXIMITY_MEMORY_LABELS: Readonly<Record<string, string>> = {
  nearMiss: "Remembered a near-miss with the other coaster",
  headchopper: "Remembered a train rushing overhead",
  dueling: "Remembered racing alongside another train",
  tunnel: "Remembered a thrilling tunnel pass",
  terrainHugging: "Remembered a coaster skimming close to the ground",
};

const PROXIMITY_THOUGHTS: Readonly<Record<string, string>> = {
  nearMiss: "That near-miss with the other coaster made me scream!",
  headchopper: "That train nearly took my hat off!",
  dueling: "Racing alongside the other train was thrilling!",
  tunnel: "Rushing through that tunnel was exciting!",
  terrainHugging: "That coaster came so close to the ground!",
};

const SCENERY_THEME_LABELS: Readonly<Record<number, string>> = {
  0: "Garden",
  1: "Woodland",
};

export function activityLabel(activity: GuestActivity): string {
  const place = activity.placeId === null ? "" : ` · stop ${activity.placeId + 1}`;
  return `${ACTIVITY_LABELS[activity.kind] ?? activity.kind}${place}`;
}

export function memoryLabel(memory: GuestMemory): string {
  switch (memory.kind) {
    case "ride": {
      const feeling = memory.enjoyment >= 0 ? "enjoyed" : "found a bit much";
      const queasy = memory.nausea > 0 ? ` · ${memory.nausea} nausea` : "";
      return `Ride ${memory.rideId + 1} — ${feeling} it (${memory.enjoyment > 0 ? "+" : ""}${memory.enjoyment} enjoyment${queasy})`;
    }
    case "food":
      return `Stopped for a bite at stall ${memory.placeId + 1}`;
    case "wait": {
      const actualDurationSeconds = Math.ceil(memory.durationTicks / 30);
      const perceivedDurationSeconds = Math.ceil(memory.perceivedDurationTicks / 30);
      const perception = actualDurationSeconds === perceivedDurationSeconds
        ? ""
        : ` (felt like ${perceivedDurationSeconds} seconds)`;
      return `Waited about ${actualDurationSeconds} seconds for stop ${memory.placeId + 1}${perception}`;
    }
    case "proximity":
      return PROXIMITY_MEMORY_LABELS[memory.eventKind] ?? "Remembered a surprising moment on a coaster";
    case "scenery": {
      const theme = SCENERY_THEME_LABELS[memory.theme] ?? "park";
      return `Enjoyed ${theme} scenery (+${memory.enjoyment} enjoyment)`;
    }
    default:
      return "Remembered a day at the park";
  }
}

export function thoughtLabel(thought: GuestThought, thoughtText: string): string {
  if (thought.kind === "proximity") {
    const data = thought.data;
    if (typeof data === "object" && data !== null) return PROXIMITY_THOUGHTS[data.eventKind] ?? "That coaster ride was full of surprises!";
    return "That coaster ride was full of surprises!";
  }
  const friendlyThoughtKinds = [
    "exploring", "hungry", "thirsty", "needsRestroom", "nauseous", "tired", "unhappy",
    "enjoyedRide", "dislikedRide", "enjoyedScenery",
  ];
  if (!friendlyThoughtKinds.includes(thought.kind)) return "I wonder what to do next.";
  if (!thoughtText.trim()) throw new Error(`Guest thought ${thought.kind} has no friendly wording`);
  return thoughtText;
}

export function relationshipLabel(relationship: string): string {
  const labels: Readonly<Record<string, string>> = {
    self: "This guest",
    groupmate: "Groupmate",
    parent: "Parent",
    child: "Child",
    partner: "Partner",
    friend: "Friend",
  };
  const label = labels[relationship];
  if (!label) throw new Error(`Unknown guest relationship: ${relationship}`);
  return label;
}

export function traitSummary(intensity: number, themingCare: number): string[] {
  const intensityLabel = intensity < 35 ? "Prefers gentle rides" : intensity > 66 ? "Loves a big thrill" : "Enjoys a mix of rides";
  const themingLabel = themingCare > 65 ? "Notices the little details" : "Here for the rides";
  return [intensityLabel, themingLabel];
}
