import { describe, expect, it } from "vitest";
import { coasterListItems } from "./coaster-list";

describe("coaster list", () => {
  it("shows each coaster name, catalog model, operating state, and completion state", () => {
    expect(coasterListItems([
      { id: 0, name: "Daisy Dash", model: "hybrid", open: true, complete: true },
      { id: 1, name: "Little Loop", model: "wooden_twister", open: false, complete: false },
    ])).toEqual([
      { id: 0, name: "Daisy Dash", model: "Timber Tumbler", status: "Open" },
      { id: 1, name: "Little Loop", model: "Briar Blitz", status: "Closed · Incomplete" },
    ]);
  });

  it("returns an empty list when the park has no coasters", () => {
    expect(coasterListItems([])).toEqual([]);
  });
});
