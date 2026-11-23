import type { Pick } from "../render/index";
const COASTER_PART = /^(track|support|footer|train)-(\d+)-\d+$/;
const MAX_COASTER_ID = 0xffff_ffff;

/** The coaster ID encoded in a picked track, support or train part. */
export function rideFromPick(pick: Pick | null): number | null {
  if (!pick || !("id" in pick) || (pick.kind !== "track" && pick.kind !== "support" && pick.kind !== "train")) return null;
  const match = COASTER_PART.exec(pick.id);
  const pickedKind = match?.[1] === "footer" ? "support" : match?.[1];
  if (!match || pickedKind !== pick.kind) return null;
  const coasterId = Number(match[2]);
  return Number.isInteger(coasterId) && coasterId <= MAX_COASTER_ID ? coasterId : null;
}
