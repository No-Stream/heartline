import { describe, expect, it } from "vitest";
import { rideOpenAction, ridePriceAction, rideRenameAction } from "./controls";

describe("ride window controls", () => {
  it("toggles only a complete, editable coaster", () => {
    expect(rideOpenAction({ ride: 12, open: false, complete: true, editable: true })).toEqual({
      type: "ride-set-open",
      ride: 12,
      open: true,
    });
    expect(rideOpenAction({ ride: 12, open: true, complete: true, editable: true })).toEqual({
      type: "ride-set-open",
      ride: 12,
      open: false,
    });
    expect(rideOpenAction({ ride: 12, open: false, complete: false, editable: true })).toBeUndefined();
    expect(rideOpenAction({ ride: 0xffff_ffff, open: true, complete: true, editable: false })).toBeUndefined();
  });

  it("accepts only whole ticket prices in the supported range", () => {
    expect(ridePriceAction(12, 0, true)).toEqual({ type: "ride-set-price", ride: 12, price: 0 });
    expect(ridePriceAction(12, 65535, true)).toEqual({ type: "ride-set-price", ride: 12, price: 65535 });
    expect(ridePriceAction(12, -1, true)).toBeUndefined();
    expect(ridePriceAction(12, 65536, true)).toBeUndefined();
    expect(ridePriceAction(12, 1.5, true)).toBeUndefined();
    expect(ridePriceAction(0xffff_ffff, 5, false)).toBeUndefined();
  });

  it("trims coaster names and ignores blank or unchanged values", () => {
    expect(rideRenameAction(12, "Old Name", "  New Name  ", true)).toEqual({
      type: "ride-rename",
      ride: 12,
      name: "New Name",
    });
    expect(rideRenameAction(12, "Old Name", "  ", true)).toBeUndefined();
    expect(rideRenameAction(12, "Same Name", " Same Name ", true)).toBeUndefined();
    expect(rideRenameAction(0xffff_ffff, "Old Name", "New Name", false)).toBeUndefined();
  });
});
