import type { Rotation } from "./camera";

const GUEST_VIEWS = ["se", "sw", "nw", "ne"] as const;

// Guests currently have no simulated heading and face world +x.
export function guestViewForRotation(rotation: Rotation): typeof GUEST_VIEWS[number] {
  return GUEST_VIEWS[rotation];
}
