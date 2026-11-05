import { describe, expect, it } from "vitest";
import { candidateForceReadout } from "./candidate-force-readout";

const preview = {
  status: "reachedEnd" as const,
  outerBank: false,
  entrySpeed: 20,
  ratedSpeed: { minimumEntry: 19, maximumEntry: 21 },
  peaks: { maximumSpeed: 22, minimumVerticalG: -0.5, maximumVerticalG: 3.4, maximumAbsoluteLateralG: 0.3, maximumAbsoluteLongitudinalG: 0.2 },
  limits: { minimumVerticalG: -1, maximumVerticalG: 4, maximumLateralG: 1, maximumLongitudinalG: 1 },
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

describe("hill force readout", () => {
  it("formats predicted hill forces and airtime in the selected units", () => {
    const hill = {
      entrySpeed: 20,
      source: "energy" as const,
      ratedSpeed: { minimumEntry: 16, maximumEntry: 26 },
      minimumVerticalG: -1, maximumVerticalG: 3.7, airtimeSeconds: 1.25,
    };
    expect(candidateForceReadout(null, "metric", hill)).toEqual([
      "Hill profile estimate (entry speed: energy estimate)",
      "Estimated entry speed: 72 km/h (within rated range 57.6–93.6 km/h)",
      "Estimated minimum vertical force: −1 g; maximum vertical force: 3.7 g",
      "Estimated airtime: 1.3 s",
    ]);
    expect(candidateForceReadout(null, "imperial", hill)[1]).toBe("Estimated entry speed: 44.7 mph (within rated range 35.8–58.2 mph)");
  });

  it("reports rollback and speeds outside the rated range", () => {
    const hill = {
      entrySpeed: 30,
      source: "energy" as const,
      ratedSpeed: { minimumEntry: 16, maximumEntry: 26 },
      minimumVerticalG: 0.2, maximumVerticalG: 4, airtimeSeconds: 0,
    };
    expect(candidateForceReadout(null, "metric", hill)[1]).toContain("outside rated range");
    const combined = candidateForceReadout(preview, "metric", { ...hill, source: "simulated" });
    expect(combined).toContain("Estimated minimum vertical force: −0.5 g (good airtime)");
    expect(combined).toContain("Hill profile estimate (entry speed: simulated)");
    expect(candidateForceReadout(null, "metric", { ...hill, minimumVerticalG: null, maximumVerticalG: null, airtimeSeconds: null })).toEqual(["Rollback expected; no hill force estimate is available."]);
    expect(candidateForceReadout(null, "metric", { ...hill, source: "timedOut", entrySpeed: null, minimumVerticalG: null, maximumVerticalG: null, airtimeSeconds: null })).toEqual(["The estimate timed out before this hill; no force estimate is available."]);
  });
});
