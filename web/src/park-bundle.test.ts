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
      "13113837825746010824",
      "4266801657982218567",
      "15252334152116589189",
      "467818520655718887",
      "2589811755385408841",
      "4833598224291227361",
      "418200132303653428",
      "5654565966350130418",
      "4131557575749932961",
      "7064741712001375895",
      "13298009551784144366",
      "8821651243896786748",
      "3497144826581369519",
      "6835268791761498747",
      "6034498671921711249",
      "16704273598063121866",
      "7093948657430582074",
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
