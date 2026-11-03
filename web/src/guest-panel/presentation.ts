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
    case "wait":
      return `Waited about ${Math.ceil(memory.durationTicks / 30)} seconds for stop ${memory.placeId + 1}`;
  }
}

export function thoughtLabel(thought: GuestThought, thoughtText: string): string {
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
