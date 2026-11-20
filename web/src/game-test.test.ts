import { expect, test } from "vitest";
import { constructionRiderPositions, readConstructionEstimates } from "./game-test";
import type { ConstructionPiece, ConstructionState } from "./construction-protocol";

test("finds rider poses when active train car IDs are not contiguous", () => {
  const poses = new Float32Array([12, 1, 2, 3, 0, 1, 0, 0, 0, 1, 0, 13, 4, 5, 6, 0, 1, 0, 0, 0, 1, 0]);
  expect(constructionRiderPositions(poses, new Uint32Array([13, 0, 42]))).toEqual([
    { car: 13, seat: 0, id: "guest-42", x: 4, y: 5, z: 6 },
  ]);
});

test("reads the selected piece and construction estimates from the latest state", () => {
  const limits: NonNullable<ConstructionState["candidateForcePreview"]>["limits"] = {
    minimumVerticalG: -2,
    maximumVerticalG: 6,
    ratingBands: { airtime: [], vertical: [], lateral: [], longitudinal: [] },
    maximumLateralG: 1,
    maximumLongitudinalG: 1,
  };
  const forcePreview: NonNullable<ConstructionState["constructionForcePreview"]> = {
    samples: [], forceEnvelopeSamples: [], pieceForcePeaks: [], completed: true, status: "reachedEnd", lastPlaced: null, launches: [], limits,
  };
  const candidateForcePreview: NonNullable<ConstructionState["candidateForcePreview"]> = {
    status: "reachedEnd", outerBank: false, entrySpeed: 12, peaks: null, samples: [], forceEnvelopeSamples: [], pieceForcePeaks: [], limits, ratedSpeed: null,
  };
  const speedMatch: NonNullable<ConstructionState["speedMatch"]> = {
    entrySpeed: 12, recommendedId: "FloaterHill16x96", fits: true, variants: [],
  };
  const hillEstimate: NonNullable<ConstructionPiece["hillEstimate"]> = {
    entrySpeed: 12,
    source: "predicted",
    ratedSpeed: null,
    minimumVerticalG: null,
    maximumVerticalG: null,
    maximumAbsoluteLateralG: null,
    airtimeSeconds: null,
    limits,
  };
  const selectedPiece: ConstructionPiece = {
    id: "FloaterHill16x96", label: "Floater hill", group: "hills", heartlined: null,
    heading: "Straight", slope: "Level", bank: "Upright", refusal: "", wouldTunnel: false, hillEstimate,
  };
  const state: Pick<ConstructionState, "selected" | "pieces" | "constructionForcePreview" | "candidateForcePreview" | "speedMatch"> = {
    selected: selectedPiece.id,
    pieces: [selectedPiece],
    constructionForcePreview: forcePreview,
    candidateForcePreview,
    speedMatch,
  };

  expect(readConstructionEstimates(state)).toEqual({
    selected: selectedPiece.id,
    selectedPiece,
    constructionForcePreview: forcePreview,
    candidateForcePreview,
    speedMatch,
    hillEstimate,
  });
});

test("returns empty construction estimates before a construction state arrives", () => {
  expect(readConstructionEstimates(undefined)).toEqual({
    selected: null,
    selectedPiece: null,
    constructionForcePreview: null,
    candidateForcePreview: null,
    speedMatch: null,
    hillEstimate: null,
  });
});

test("preserves selected element ids without inventing a piece or hill estimate", () => {
  const state: Pick<ConstructionState, "selected" | "pieces" | "constructionForcePreview" | "candidateForcePreview" | "speedMatch"> = {
    selected: "element:immelmann-left",
    pieces: [],
    constructionForcePreview: null,
    candidateForcePreview: null,
    speedMatch: null,
  };

  expect(readConstructionEstimates(state)).toMatchObject({
    selected: "element:immelmann-left",
    selectedPiece: null,
    hillEstimate: null,
  });
});
