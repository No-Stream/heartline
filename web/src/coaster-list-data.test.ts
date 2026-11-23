import { describe, expect, it } from "vitest";
import { DEMO_COASTER_ID, type CoasterListEntry } from "./protocol";
import { coasterListEntries } from "./coaster-list-data";

describe("coaster list data", () => {
  it("includes the reserved demo coaster as a closed, read-only summary", () => {
    const playerCoaster: CoasterListEntry = {
      id: 0,
      name: "Daisy Dash",
      model: "family_coaster",
      open: true,
      price: 12,
      income: 30,
      queueLength: 4,
      complete: true,
    };

    expect(coasterListEntries([playerCoaster], [{ id: DEMO_COASTER_ID, trainLayout: { model: "hybrid" } }])).toEqual([
      playerCoaster,
      {
        id: DEMO_COASTER_ID,
        name: "Timber Tumbler",
        model: "hybrid",
        open: false,
        price: 0,
        income: 0,
        queueLength: 0,
        complete: true,
      },
    ]);
  });

  it("leaves ordinary parks unchanged when their render list has no demo coaster", () => {
    const playerCoaster: CoasterListEntry = {
      id: 0,
      name: "Daisy Dash",
      model: "family_coaster",
      open: true,
      price: 12,
      income: 30,
      queueLength: 4,
      complete: true,
    };

    expect(coasterListEntries([playerCoaster], [{ id: 0, trainLayout: { model: "family_coaster" } }])).toEqual([playerCoaster]);
  });
});
