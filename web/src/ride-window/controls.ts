import type { InputAction } from "../input/actions";

export interface RideControlState {
  readonly ride: number;
  readonly open: boolean;
  readonly complete: boolean;
  readonly editable: boolean;
}

const MAX_RIDE_PRICE = 65535;

export function rideOpenAction(state: RideControlState): InputAction | undefined {
  if (!state.editable || !state.complete) return undefined;
  return { type: "ride-set-open", ride: state.ride, open: !state.open };
}

export function ridePriceAction(ride: number, price: number, editable: boolean): InputAction | undefined {
  if (!editable || !Number.isInteger(price) || price < 0 || price > MAX_RIDE_PRICE) return undefined;
  return { type: "ride-set-price", ride, price };
}

export function rideRenameAction(ride: number, currentName: string, candidateName: string, editable: boolean): InputAction | undefined {
  const name = candidateName.trim();
  if (!editable || name === "" || name === currentName) return undefined;
  return { type: "ride-rename", ride, name };
}
