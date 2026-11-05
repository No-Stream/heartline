import type { InputAction } from "./input/actions";

/** Serializes player intent against the worker's authoritative construction state. */
export function createConstructionInputQueue(
  execute: (action: InputAction) => boolean,
  showNotice: (notice: string) => void,
  limit = 32,
) {
  const waiting: InputAction[] = [];
  let inFlight: { action: InputAction; trackLength: number; cancelled: boolean } | undefined;
  let busy = false;
  let trackLength = 0;

  function showPending(): void {
    showNotice(waiting.length ? `${waiting.length} ${waiting.length === 1 ? "action" : "actions"} queued` : "");
  }

  function drain(): void {
    while (!busy && !inFlight && waiting.length) {
      const action = waiting.shift()!;
      if (execute(action)) inFlight = { action, trackLength, cancelled: false };
    }
    showPending();
  }

  return {
    submit(action: InputAction, currentTrackLength: number): void {
      if (!inFlight && !busy) trackLength = currentTrackLength;
      if (waiting.length >= limit) {
        showNotice("Input queue full; action not accepted.");
        return;
      }
      waiting.push(action);
      drain();
    },
    setBusy(value: boolean, currentTrackLength: number): void {
      busy = value;
      if (!inFlight) trackLength = currentTrackLength;
      if (!busy) drain();
    },
    complete(currentTrackLength: number, active: boolean): void {
      if (!inFlight) return;
      const refused = !inFlight.cancelled && inFlight.action.type === "construction-append" && currentTrackLength <= inFlight.trackLength;
      inFlight = undefined;
      trackLength = currentTrackLength;
      if (!active || refused) {
        const cancelled = waiting.length;
        waiting.length = 0;
        if (cancelled) showNotice(refused ? "Placement refused; queued actions cancelled." : "Editor closed; queued actions cancelled.");
        else showPending();
        return;
      }
      drain();
    },
    cancel(): void {
      if (inFlight) inFlight.cancelled = true;
      const cancelled = waiting.length;
      waiting.length = 0;
      if (cancelled) showNotice("Editor closed; queued actions cancelled.");
    },
    pending(): number { return waiting.length; },
  };
}
