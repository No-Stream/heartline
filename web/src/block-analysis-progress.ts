import type { BlockSection } from "./construction-protocol";

export function blockLayoutChanged(previous: readonly BlockSection[], next: readonly BlockSection[]): boolean {
  return JSON.stringify(previous) !== JSON.stringify(next);
}
