import type { Pick } from "../render/index";
import type { RideKey } from "./protocol";

const COASTER_PART = /^(?:(construction-)?)(?:track|support|train)-\d+$/;

/** The coaster a picked track, support or train part belongs to. The player's coaster is drawn with a "construction-" id prefix. */
export function rideFromPick(pick: Pick | null): RideKey | null {
  if (!pick || !("id" in pick) || (pick.kind !== "track" && pick.kind !== "support" && pick.kind !== "train")) return null;
  const match = COASTER_PART.exec(pick.id);
  if (!match) return null;
  return match[1] ? "player" : "demo";
}
