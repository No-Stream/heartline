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

  it("preserves full precision for shaped geometry digests in the embedded park", () => {
    const bundledPark = JSON.parse(bundledParkDescriptionJson("horizon-hawk")) as {
      coasters: {
        ride: {
          steps: { intent?: unknown; spec?: { geometry_digest?: unknown } }[];
        };
      }[];
    };
    const shapedDigests = bundledPark.coasters[0]?.ride.steps
      .filter((step) => step.intent !== undefined)
      .map((step) => step.spec?.geometry_digest);

    expect(shapedDigests).toEqual([
      "1099196092358164685",
      "1543303591791284866",
      "7214862775167386627",
      "11147926741504291590",
      "17780804300636241268",
      "9945719657027232816",
      "274810590240427382",
      "1973078856447352676",
      "18244297142596138433",
      "15350274490597005740",
      "17892890610725569961",
      "12597791983754431263",
      "701253095934839283",
      "7655754950869405580",
      "4199539064089093963",
      "17564952306928190045",
      "11919310966154575487",
      "586703719065714703",
    ]);
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
