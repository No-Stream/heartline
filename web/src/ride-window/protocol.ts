import type { RideInfoReport } from "../generated/RideInfoReport";

export type RideKey = number;
export type RideInfo = Omit<RideInfoReport, "ride"> & { readonly ride: RideKey };
