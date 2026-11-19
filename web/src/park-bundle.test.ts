import { describe, expect, it } from "vitest";
import { bundleParkDescriptionJson, bundledParkDescriptionJson } from "./park-bundle";

describe("bundled park descriptions", () => {
  it("inlines referenced ride files into the park sent to Wasm", () => {
    const bundledPark = JSON.parse(bundledParkDescriptionJson("horizon-hawk")) as {
      coasters: { ride: { format: string; name: string } }[];
    };

    expect(bundledPark.coasters[0]?.ride).toMatchObject({
      format: "heartline-ride",
      name: "Horizon Hawk",
    });
  });

  it("fails loudly when a park name is not bundled", () => {
    expect(() => bundledParkDescriptionJson("missing-park")).toThrow(/Unknown park description/);
  });

  it("rejects unsupported multi-coaster parks before resolving ride files", () => {
    expect(() => bundleParkDescriptionJson("two-coasters", {
      coasters: [
        { ride: "content/rides/missing-first.ride.json" },
        { ride: "content/rides/missing-second.ride.json" },
      ],
    }, {})).toThrow(/this build supports one player coaster per park/);
  });
});
