import { describe, expect, it } from "vitest";
import { candidateForceReadout, hillReadout } from "./candidate-force-readout";
import { MODEL_FORCE_LIMITS } from "./generated/modelForceLimits";

const preview = {
  status: "reachedEnd" as const,
  outerBank: false,
  entrySpeed: 20,
  ratedSpeed: { minimumEntry: 19, maximumEntry: 21 },
  peaks: { maximumSpeed: 22, minimumVerticalG: -0.5, maximumVerticalG: 3.4, maximumAbsoluteLateralG: 0.3, maximumAbsoluteLongitudinalG: 0.2 },
  limits: MODEL_FORCE_LIMITS["Briar Blitz"],
};

describe("candidate force readout", () => {
  it("shows estimated entry speed and peak forces in the chosen units", () => {
    expect(candidateForceReadout(preview, "metric")).toEqual([
      "Front-car force estimates (sampled along the track)",
      "Estimated entry speed: 72 km/h (within rated range 68.4–75.6 km/h)",
      "Estimated minimum vertical force: −0.5 g (good airtime)",
      "Estimated maximum vertical force: 3.4 g (great)",
      "Estimated lateral force: 0.3 g (fine); longitudinal force: 0.2 g (fine)",
    ]);
    expect(candidateForceReadout(preview, "imperial")[1]).toBe("Estimated entry speed: 44.7 mph (within rated range 42.5–47 mph)");
  });

  it("labels outer-bank lateral force alongside the vertical force bands", () => {
    expect(candidateForceReadout({ ...preview, outerBank: true }, "metric")).toEqual([
      "Front-car force estimates (sampled along the track)",
      "Estimated entry speed: 72 km/h (within rated range 68.4–75.6 km/h)",
      "Estimated minimum vertical force: −0.5 g (good airtime)",
      "Estimated maximum vertical force: 3.4 g (great)",
      "Estimated outer-bank lateral force: 0.3 g (fine); longitudinal force: 0.2 g (fine)",
    ]);
  });

  it("reports when no size fits or the train cannot reach the piece", () => {
    expect(candidateForceReadout({ ...preview, entrySpeed: 25 }, "metric")[1]).toContain("outside rated range");
    expect(candidateForceReadout({ ...preview, status: "stalled", entrySpeed: null, peaks: null }, "metric"))
      .toEqual(["The train does not reach this piece; no force estimate is available."]);
    expect(candidateForceReadout({ ...preview, status: "stalled", peaks: null }, "metric"))
      .toEqual(["The train reaches this piece, but no force samples are available."]);
  });

  it("marks forces as partial when the candidate stalls or times out", () => {
    expect(candidateForceReadout({ ...preview, status: "stalled" }, "metric")[0]).toBe("The train stalls in this piece; force estimates are partial.");
    expect(candidateForceReadout({ ...preview, status: "timedOut" }, "metric")[0]).toBe("The estimate timed out in this piece; force estimates are partial.");
  });
});

describe("airtime hill readout", () => {
  const hill = {
    entrySpeed: 20,
    source: "energy",
    ratedSpeed: { minimumEntry: 16, maximumEntry: 26 },
    minimumVerticalG: -1.1, maximumVerticalG: 3.4, airtimeSeconds: 1.25, maximumAbsoluteLateralG: null,
    limits: MODEL_FORCE_LIMITS["Timber Tumbler"],
  };

  it("rates the predicted forces and shows speeds in the chosen units", () => {
    expect(hillReadout(hill, "metric")).toEqual({
      rows: [
        { label: "Entry speed", value: "72 km/h", note: "", tone: null },
        { label: "Rated for", value: "58–94 km/h", note: "in range", tone: "fine" },
        { label: "Lowest", value: "−1.1 g", note: "great airtime", tone: "great" },
        { label: "Highest", value: "3.4 g", note: "great", tone: "great" },
        { label: "Airtime", value: "1.3 s", note: "", tone: null },
      ],
      message: "",
    });
    const imperial = hillReadout(hill, "imperial").rows;
    expect(imperial[0].value).toBe("45 mph");
    expect(imperial[1].value).toBe("36–58 mph");
  });

  it("shows the outer bank lateral force", () => {
    expect(hillReadout({ ...hill, maximumAbsoluteLateralG: 1.1 }, "metric").rows).toContainEqual(
      { label: "Lateral", value: "1.1 g", note: "strong", tone: "great" },
    );
  });

  it("flags speeds outside the rated window and forces past the limit", () => {
    const fast = hillReadout({ ...hill, entrySpeed: 27, minimumVerticalG: -1.8, maximumVerticalG: 6 }, "metric").rows;
    expect(fast[1]).toMatchObject({ note: "too fast", tone: "past" });
    expect(fast[2]).toMatchObject({ note: "too much airtime", tone: "past" });
    expect(fast[3]).toMatchObject({ note: "dangerous", tone: "dangerous" });
    expect(hillReadout({ ...hill, entrySpeed: 15, minimumVerticalG: -0.1 }, "metric").rows.slice(1, 3).map(({ note }) => note)).toEqual(["too slow", "weak airtime"]);
  });

  it("explains when there is no force estimate", () => {
    const noForces = { minimumVerticalG: null, maximumVerticalG: null, airtimeSeconds: null };
    expect(hillReadout({ ...hill, ...noForces, entrySpeed: 9 }, "metric")).toEqual({
      rows: [{ label: "Entry speed", value: "32 km/h", note: "", tone: null }, { label: "Rated for", value: "58–94 km/h", note: "too slow", tone: "past" }],
      message: "At this speed the train would not make it over the crest.",
    });
    expect(hillReadout({ ...hill, ...noForces, entrySpeed: null }, "metric").message).toBe("The train would roll back before reaching this hill.");
    expect(hillReadout({ ...hill, ...noForces, entrySpeed: 0, source: "stalled" }, "metric").message).toBe("The train stalls before reaching this hill.");
    expect(hillReadout({ ...hill, ...noForces, entrySpeed: null, source: "timedOut" }, "metric")).toEqual({
      rows: [{ label: "Rated for", value: "58–94 km/h", note: "", tone: null }],
      message: "The speed estimate timed out before this hill.",
    });
  });
});
