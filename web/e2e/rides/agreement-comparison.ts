import type { TestingReport } from "../../src/generated/TestingReport";

/** The comparisons the browser ride-agreement test and the fast Node/WASM agreement check both apply. */

export type NativeRideCheck = {
  readonly name: string;
  readonly expectationFailures: readonly string[];
  readonly expectations: readonly {
    readonly metric: string;
    readonly observed: unknown;
    readonly passed: boolean;
  }[];
  readonly browserEstimates: readonly NativeBrowserEstimate[];
  readonly agreement: {
    readonly passed: boolean;
    readonly rows: readonly {
      readonly pieceIndex: number;
      readonly pieceName: string;
      readonly finishedEntrySpeedEstimate: number | null;
      readonly entrySpeedMeasuredFront: number | null;
      readonly placementEstimate: ForcePeaks | null;
      readonly finishedTrackEstimate: ForcePeaks | null;
      readonly finishedTrackWorstCarEstimate: ForcePeaks | null;
      readonly measuredFront: ForcePeaks | null;
      readonly measuredWorst: ForcePeaks | null;
      readonly check2: "Passed" | "Failed";
      readonly check3: "Passed" | "Failed";
    }[];
    readonly failures: readonly {
      readonly pieceIndex: number;
      readonly pieceName: string;
      readonly check: string;
      readonly metric: string;
      readonly reason: string;
      readonly classification?: "downstream track" | "estimate error";
      readonly cause?: string;
    }[];
  };
};

export type NativeRideCheckWithSamples = NativeRideCheck & {
  readonly reportSamples: readonly NativeReportSample[];
};

export type NativeReportSample = {
  readonly piece_index: number;
  readonly vertical_g: number;
  readonly lateral_g: number;
  readonly longitudinal_g: number;
  readonly distance: number;
  readonly speed: number;
};

export type BrowserReportSample = {
  readonly pieceIndex: number;
  readonly distance: number;
  readonly speed: number;
  readonly verticalG: number;
  readonly lateralG: number;
  readonly longitudinalG: number;
};

export type NativeBrowserEstimate = {
  readonly stepIndex: number;
  readonly pieceIndex: number;
  readonly pieceCount: number;
  readonly pieceId: string;
  readonly selection: BrowserEstimateSnapshot;
  readonly placement: BrowserEstimateSnapshot;
};

export type BrowserEstimateSnapshot = {
  readonly selected: string;
  readonly selectedShape: { readonly spec: { readonly geometryDigest: string } | null } | null;
  readonly constructionForcePreview: unknown;
  readonly candidateForcePreview: unknown;
  readonly speedMatch: unknown;
  readonly hillEstimate: unknown;
};

export type ForcePeaks = {
  readonly minimumVerticalG: number;
  readonly maximumVerticalG: number;
  readonly maximumLateralG: number;
  readonly maximumLongitudinalG: number;
};

export function firstMismatch(expected: unknown, actual: unknown, path: string): string | null {
  if (typeof expected === "number") {
    if (typeof actual !== "number") return `${path} must be numeric; received ${typeof actual}`;
    const difference = Math.abs(expected - actual);
    if (difference > 1e-6 || !Number.isFinite(difference)) {
      return `${path} differs by ${difference.toExponential(2)} (expected ${expected}, received ${actual})`;
    }
    return null;
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) return `${path} must be an array`;
    const actualValues = actual as unknown[];
    if (actualValues.length !== expected.length) return `${path} length differs (expected ${expected.length}, received ${actualValues.length})`;
    for (let index = 0; index < expected.length; index += 1) {
      const mismatch = firstMismatch(expected[index], actualValues[index], `${path}[${index}]`);
      if (mismatch) return mismatch;
    }
    return null;
  }
  if (expected !== null && typeof expected === "object") {
    if (actual === null || typeof actual !== "object" || Array.isArray(actual)) return `${path} must be an object`;
    const expectedObject = expected as Record<string, unknown>;
    const actualObject = actual as Record<string, unknown>;
    const expectedKeys = Object.keys(expectedObject).sort();
    const actualKeys = Object.keys(actualObject).sort();
    if (expectedKeys.length !== actualKeys.length || expectedKeys.some((key, index) => key !== actualKeys[index])) {
      return `${path} fields differ (expected ${expectedKeys.join(", ")}, received ${actualKeys.join(", ")})`;
    }
    for (const key of expectedKeys) {
      const mismatch = firstMismatch(expectedObject[key], actualObject[key], `${path}.${key}`);
      if (mismatch) return mismatch;
    }
    return null;
  }
  if (!Object.is(actual, expected)) return `${path} differs (expected ${String(expected)}, received ${String(actual)})`;
  return null;
}

export function browserAgreementResults(nativeCheck: NativeRideCheckWithSamples, report: TestingReport): {
  readonly passedPieces: number;
  readonly totalPieces: number;
  readonly failures: readonly string[];
  readonly browserToNativeSampleDifferences: readonly string[];
  readonly sampleIdentityValid: boolean;
} {
  const nativeReportSamples = nativeCheck.reportSamples;
  if (nativeReportSamples.length !== report.samples.length) {
    return {
      passedPieces: 0,
      totalPieces: nativeCheck.agreement.rows.length,
      failures: [`test report sample count differs: native ${nativeReportSamples.length}, browser ${report.samples.length}`],
      browserToNativeSampleDifferences: [],
      sampleIdentityValid: false,
    };
  }
  const samplesByPiece = new Map<number, BrowserReportSample[]>();
  const browserPeaksByPiece = new Map(report.pieceForcePeaks.map((piece) => [piece.pieceIndex, piece]));
  if (browserPeaksByPiece.size !== report.pieceForcePeaks.length) {
    return {
      passedPieces: 0,
      totalPieces: nativeCheck.agreement.rows.length,
      failures: ["browser testing report contains duplicate per-piece peak entries"],
      browserToNativeSampleDifferences: [],
      sampleIdentityValid: true,
    };
  }
  const sampleMismatches: string[] = [];
  const identityFailures: string[] = [];
  nativeReportSamples.forEach((nativeSample, index) => {
    const browserSample = report.samples[index];
    if (Math.abs(nativeSample.distance - browserSample.distance) > 1e-6) {
      identityFailures.push(`sample ${index + 1} distance differs by ${Math.abs(nativeSample.distance - browserSample.distance).toExponential(2)}`);
    }
    for (const [name, expected, actual] of [
      ["speed", nativeSample.speed, browserSample.speed],
      ["vertical g", nativeSample.vertical_g, browserSample.verticalG],
      ["lateral g", nativeSample.lateral_g, browserSample.lateralG],
      ["longitudinal g", nativeSample.longitudinal_g, browserSample.longitudinalG],
    ] as const) {
      if (Math.abs(expected - actual) > 1e-6) {
        sampleMismatches.push(`sample ${index + 1} ${name} differs by ${Math.abs(expected - actual).toExponential(2)}`);
      }
    }
  });
  if (identityFailures.length > 0) {
    return {
      passedPieces: 0,
      totalPieces: nativeCheck.agreement.rows.length,
      failures: [`browser/native report samples cannot be paired by recorder order: ${identityFailures.join("; ")}`],
      browserToNativeSampleDifferences: sampleMismatches,
      sampleIdentityValid: false,
    };
  }
  if (sampleMismatches.length > 0) {
    return {
      passedPieces: 0,
      totalPieces: nativeCheck.agreement.rows.length,
      failures: [`${sampleMismatches.length} browser/native report sample values differ: ${sampleMismatches.slice(0, 5).join("; ")}`],
      browserToNativeSampleDifferences: sampleMismatches,
      sampleIdentityValid: true,
    };
  }

  nativeReportSamples.forEach((nativeSample, index) => {
    const browserSample = report.samples[index];
    const pieceSamples = samplesByPiece.get(nativeSample.piece_index) ?? [];
    pieceSamples.push({
      pieceIndex: nativeSample.piece_index,
      distance: browserSample.distance,
      speed: browserSample.speed,
      verticalG: browserSample.verticalG,
      lateralG: browserSample.lateralG,
      longitudinalG: browserSample.longitudinalG,
    });
    samplesByPiece.set(nativeSample.piece_index, pieceSamples);
  });

  const failedPieces = new Set<number>();
  const failures: string[] = [];
  for (const row of nativeCheck.agreement.rows) {
    const pieceSamples = samplesByPiece.get(row.pieceIndex);
    const browserPeaks = browserPeaksByPiece.get(row.pieceIndex);
    if (
      !pieceSamples?.length ||
      !browserPeaks ||
      !row.measuredFront ||
      !row.measuredWorst ||
      !row.finishedTrackEstimate ||
      !row.finishedTrackWorstCarEstimate ||
      !row.placementEstimate ||
      row.finishedEntrySpeedEstimate === null ||
      row.entrySpeedMeasuredFront === null
    ) {
      failures.push(`piece ${row.pieceIndex} ${row.pieceName}: browser report samples or per-piece peaks unavailable`);
      failedPieces.add(row.pieceIndex);
      continue;
    }
    if (!browserPeaks.frontCar || !browserPeaks.worstCar) {
      failures.push(`piece ${row.pieceIndex} ${row.pieceName}: browser report omitted front-car or worst-car peaks`);
      failedPieces.add(row.pieceIndex);
      continue;
    }
    if (Math.abs(row.entrySpeedMeasuredFront - pieceSamples[0].speed) > 1e-6) {
      failures.push(`piece ${row.pieceIndex} entry speed measurement: native ${row.entrySpeedMeasuredFront.toFixed(3)} vs browser front car ${pieceSamples[0].speed.toFixed(3)} m/s`);
      failedPieces.add(row.pieceIndex);
    }
    if (Math.abs(row.finishedEntrySpeedEstimate - pieceSamples[0].speed) > 0.5) {
      failures.push(`piece ${row.pieceIndex} finished entry speed: estimate ${row.finishedEntrySpeedEstimate.toFixed(3)} vs browser front car ${pieceSamples[0].speed.toFixed(3)} m/s`);
      failedPieces.add(row.pieceIndex);
    }
    for (const [car, actual, expected] of [
      ["front car", browserPeaks.frontCar, row.measuredFront],
      ["worst car", browserPeaks.worstCar, row.measuredWorst],
    ] as const) {
      for (const axis of Object.keys(expected) as (keyof ForcePeaks)[]) {
        const difference = Math.abs(expected[axis] - actual[axis]);
        if (difference > 1e-6) {
          failures.push(`piece ${row.pieceIndex} ${car} ${axis}: browser/native report differs by ${difference.toExponential(2)} g`);
          failedPieces.add(row.pieceIndex);
        }
      }
    }
    for (const [car, estimate, measured] of [
      ["front car", row.finishedTrackEstimate, browserPeaks.frontCar],
      ["worst car", row.finishedTrackWorstCarEstimate, browserPeaks.worstCar],
    ] as const) {
      for (const axis of Object.keys(measured) as (keyof ForcePeaks)[]) {
        const difference = Math.abs(estimate[axis] - measured[axis]);
        if (difference > 0.2) {
          failures.push(`piece ${row.pieceIndex} finished estimate vs ${car} ${axis}: browser estimate differs from measured force by ${difference.toFixed(3)} g, over 0.2 g`);
          failedPieces.add(row.pieceIndex);
        }
      }
    }
    if (row.check2 !== "Passed") {
      failures.push(`piece ${row.pieceIndex} ${row.pieceName}: native check 2 failed (${row.check2})`);
      failedPieces.add(row.pieceIndex);
    }
  }
  return {
    passedPieces: nativeCheck.agreement.rows.length - failedPieces.size,
    totalPieces: nativeCheck.agreement.rows.length,
    failures,
    browserToNativeSampleDifferences: sampleMismatches,
    sampleIdentityValid: true,
  };
}
