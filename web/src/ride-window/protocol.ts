import type { RideInfoReport } from "../generated/RideInfoReport";

export type RideKey = number;
export type RideInfo = Omit<RideInfoReport, "ride"> & {
  readonly ride: RideKey;
  readonly name: string;
  readonly open: boolean;
  readonly price: number;
  readonly income: number;
  readonly complete: boolean;
  readonly editable: boolean;
};
