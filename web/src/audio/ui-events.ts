import { constructionEvent } from "../construction-effects";
import type { ConstructionState } from "../construction-protocol";
import type { PlacementOutcome } from "../protocol";
import { trainMaterial, type UiEvent, type UiSound } from "./mapping";

export const UI_DEBOUNCE_MS = 60;

/** Builder messages that report progress rather than refuse something, so they get no error sound. */
const PROGRESS_MESSAGES = new Set(["Test train running.", "Review the connection, then confirm to finish the coaster."]);

/** Recognises the builder sounds from consecutive builder states: a placed piece, a closed circuit, an undo or delete, and a refusal. */
export function constructionUiEvents(previous: ConstructionState | undefined, next: ConstructionState): UiEvent[] {
  if (!previous?.active || !next.active) return [];
  const event = constructionEvent(previous, next);
  if (event === "placed") {
    if (!next.trainLayout) throw new Error("A placed track piece arrived without the coaster's train layout");
    return [{ sound: "place", material: trainMaterial(next.trainLayout) }];
  }
  if (event === "closed") return [{ sound: "finish" }];
  if (next.track.length < previous.track.length) return [{ sound: "undo" }];
  const refused = next.message !== "" && next.message !== previous.message && !PROGRESS_MESSAGES.has(next.message);
  return refused && next.track.length === previous.track.length ? [{ sound: "error" }] : [];
}

export function pathUiEvent(result: { readonly command: "build" | "remove"; readonly success: boolean }): UiEvent {
  if (!result.success) return { sound: "error" };
  return { sound: result.command === "build" ? "build" : "undo" };
}

/** Scenery and facility results; `null` is an update that was not a player's placement or removal. */
export function placementUiEvent(outcome: PlacementOutcome | null): UiEvent | null {
  if (outcome === null) return null;
  return { sound: outcome === "placed" ? "build" : outcome === "removed" ? "undo" : "error" };
}

/** Terrain edits stay silent when they succeed, since a brush drag sends many of them. */
export function terrainUiEvent(result: { readonly success: boolean }): UiEvent | null {
  return result.success ? null : { sound: "error" };
}

/** Collects interface events between frames and drops a repeat of the same sound within `UI_DEBOUNCE_MS`. */
export function createUiEventQueue() {
  let pending: UiEvent[] = [];
  const lastQueuedAt = new Map<UiSound, number>();
  return {
    push(event: UiEvent, now: number): void {
      const last = lastQueuedAt.get(event.sound);
      if (last !== undefined && now - last < UI_DEBOUNCE_MS) return;
      lastQueuedAt.set(event.sound, now);
      pending.push(event);
    },
    drain(): UiEvent[] {
      const events = pending;
      pending = [];
      return events;
    },
  };
}
