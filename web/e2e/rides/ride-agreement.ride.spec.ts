import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type TestInfo } from "@playwright/test";
import { candidateForceReadout, hillReadout } from "../../src/candidate-force-readout";
import type { TestingReport } from "../../src/generated/TestingReport";
import type { CoasterModelKey } from "../../src/generated/coaster-models";
import { placeStation, showBuilderTab, startPark, testRide, waitForState } from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { expectRideReportExplained } from "../player/ride-report";
import { readRide, type RideFile } from "../player/rides";

const repositoryRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const ridesDirectory = resolve(repositoryRoot, "content/rides");
const rideStems = readdirSync(ridesDirectory)
  .filter((filename) => filename.endsWith(".ride.json"))
  .map((filename) => filename.slice(0, -".ride.json".length));
const requestedRide = process.env.RIDE?.split("/").at(-1)?.replace(/\.ride\.json$/, "").toLocaleLowerCase();
const selectedRideStems = rideStems.filter((stem) => {
  if (!requestedRide) return true;
  const ride = readRide(stem);
  return stem.toLocaleLowerCase() === requestedRide || ride.name.toLocaleLowerCase() === requestedRide;
});

if (requestedRide && selectedRideStems.length === 0) {
  throw new Error(`RIDE=${process.env.RIDE} does not match a content/rides fixture name or filename`);
}

const rides = selectedRideStems.map((stem) => ({ stem, ride: readRide(stem) }));

const newParkSurfaceHeight = 8;
const browserAgreementFastRide = "browser-agreement-fast";

for (const { stem, ride } of rides) {
  test(`${ride.name} estimate and report agree with the builder${stem === browserAgreementFastRide ? " @fast" : ""}`, async ({ page }, testInfo) => {
    test.setTimeout(360_000);
    const nativeCheck = runNativeCheck(stem);
    const pageErrors: string[] = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));

    await startPark(page, { park: "new", seed: 149, speed: 1 });
    await placeStation(page, {
      model: ride.model as CoasterModelKey,
      height: ride.station.z - newParkSurfaceHeight,
      tile: { x: ride.station.x, y: ride.station.y, z: 2 },
      direction: stationDirection(ride.station.x, ride.station.y, ride.station.heading),
    });

    const expectedByStep = new Map(nativeCheck.browserEstimates.map((estimate) => [estimate.stepIndex, estimate]));
    let estimateComparisons = 0;
    await placeCircuit(page, ride.steps, {
      carsPerTrain: ride.carsPerTrain,
      onEstimate: async (phase, step, index) => {
        if (!("place" in step)) throw new Error(`Estimate callback for non-placement step ${index + 1}`);
        const expected = expectedByStep.get(index);
        if (!expected) throw new Error(`Native estimate missing for ride step ${index + 1} (${step.place})`);
        const nativeSnapshot = expected[phase === "selected" ? "selection" : "placement"];
        const browserView = await page.evaluate(() => {
          const requireElement = <T extends Element>(selector: string): T => {
            const element = document.querySelector<T>(selector);
            if (!element) throw new Error(`Missing builder readout ${selector}`);
            return element;
          };
          const builder = requireElement<HTMLElement>("[data-construction]");
          const candidatePanel = builder.querySelector<HTMLElement>("[data-candidate-force-preview]");
          const hillPanel = builder.querySelector<HTMLElement>("[data-hill-readout]");
          const hillMessage = builder.querySelector<HTMLElement>("[data-hill-message]");
          if (!candidatePanel || !hillPanel || !hillMessage) throw new Error("Missing builder estimate readouts");
          const textOf = (element: Element): string => {
            if (element.textContent === null) throw new Error("Builder estimate readout has null text");
            return element.textContent;
          };
          const hillRows = Array.from(hillPanel.querySelectorAll(":scope > div"), (row) => {
            const label = row.querySelector("dt");
            const value = row.querySelector("dd");
            if (!label || !value) throw new Error("Hill readout row is missing its label or value");
            return {
              label: textOf(label).trim(),
              value: textOf(value).replace(/\s+/g, " ").trim(),
            };
          });
          return {
            estimate: window.gameTest.constructionEstimates(),
            readouts: {
              candidateHidden: candidatePanel.hidden,
              candidateLines: Array.from(candidatePanel.querySelectorAll("p"), (paragraph) => textOf(paragraph).trim()),
              hillHidden: hillPanel.hidden,
              hillRows,
              hillMessageHidden: hillMessage.hidden,
              hillMessage: textOf(hillMessage).trim(),
            },
          };
        });
        const browserSnapshot = browserView.estimate;
        const browserWire = {
          selected: browserSnapshot.selected,
          selectedPieceId: browserSnapshot.selectedPiece?.id ?? null,
          constructionForcePreview: browserSnapshot.constructionForcePreview,
          candidateForcePreview: browserSnapshot.candidateForcePreview,
          speedMatch: browserSnapshot.speedMatch,
          hillEstimate: browserSnapshot.hillEstimate,
        };
        const expectedWire = {
          ...nativeSnapshot,
          selectedPieceId: nativeSnapshot.selected.startsWith("element:") ? null : nativeSnapshot.selected,
        };
        const mismatch = firstMismatch(expectedWire, browserWire, `${phase} estimate at ride step ${index + 1}`);
        expect(mismatch, `${phase} estimate at ride step ${index + 1}`).toBeNull();
        expectFormattedReadouts(browserView.readouts, browserSnapshot);
        estimateComparisons += 1;
      },
    });
    expect(estimateComparisons).toBe(nativeCheck.browserEstimates.length * 2);

    await showBuilderTab(page, "Build");
    await expect(page.getByRole("button", { name: "Test coaster" })).toBeEnabled();
    await testRide(page);
    await waitForState(page, async () => !!(await page.evaluate(() => window.gameTest.testingReport())), { ticks: 40_000 });

    const report = await expectRideReportExplained(page);
    expectRideTargets(ride, report);
    await reportCheckResults(testInfo, ride, nativeCheck, report);
    await page.screenshot({ path: testInfo.outputPath(`${stem}-ride.png`), fullPage: true });

    expect(pageErrors).toEqual([]);
  });
}

type NativeRideCheck = {
  readonly name: string;
  readonly expectationFailures: readonly string[];
  readonly browserEstimates: readonly NativeBrowserEstimate[];
  readonly agreement: {
    readonly passed: boolean;
    readonly rows: readonly {
      readonly pieceIndex: number;
      readonly pieceName: string;
      readonly entrySpeedEstimate: number | null;
      readonly entrySpeedMeasuredFront: number | null;
      readonly placementEstimate: ForcePeaks | null;
      readonly finishedTrackEstimate: ForcePeaks | null;
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
    }[];
  };
};

type NativeRideCheckWithSamples = NativeRideCheck & {
  readonly reportSamples: readonly NativeReportSample[];
};

type NativeReportSample = {
  readonly piece_index: number;
  readonly vertical_g: number;
  readonly lateral_g: number;
  readonly longitudinal_g: number;
  readonly distance: number;
  readonly speed: number;
};

type NativeRideTest = {
  readonly testRun: {
    readonly samples: readonly NativeReportSample[];
  };
};

type BrowserReportSample = {
  readonly pieceIndex: number;
  readonly distance: number;
  readonly speed: number;
  readonly verticalG: number;
  readonly lateralG: number;
  readonly longitudinalG: number;
};

type NativeBrowserEstimate = {
  readonly stepIndex: number;
  readonly pieceIndex: number;
  readonly pieceCount: number;
  readonly pieceId: string;
  readonly selection: BrowserEstimateSnapshot;
  readonly placement: BrowserEstimateSnapshot;
};

type BrowserEstimateSnapshot = {
  readonly selected: string;
  readonly constructionForcePreview: unknown;
  readonly candidateForcePreview: unknown;
  readonly speedMatch: unknown;
  readonly hillEstimate: unknown;
};

type ForcePeaks = {
  readonly minimumVerticalG: number;
  readonly maximumVerticalG: number;
  readonly maximumLateralG: number;
  readonly maximumLongitudinalG: number;
};

function stationDirection(x: number, y: number, heading: RideFile["station"]["heading"]): { x: number; y: number; z: number } {
  switch (heading) {
    case "east": return { x: x + 2, y, z: 2 };
    case "north": return { x, y: y + 2, z: 2 };
    case "west": return { x: x - 2, y, z: 2 };
    case "south": return { x, y: y - 2, z: 2 };
  }
}

function runNativeCheck(stem: string): NativeRideCheckWithSamples {
  const ridePath = resolve(ridesDirectory, `${stem}.ride.json`);
  const output = execFileSync(resolve(repositoryRoot, "tools/design"), [
    "check",
    "--json",
    ridePath,
  ], {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  const [result] = JSON.parse(output) as NativeRideCheck[];
  if (!result) throw new Error(`Native design check omitted ${stem}`);
  const testOutput = execFileSync(resolve(repositoryRoot, "tools/design"), ["ride", ridePath, "test", "--json"], {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  const rideTest = JSON.parse(testOutput) as NativeRideTest;
  if (!rideTest.testRun) throw new Error(`Native test report omitted ${stem}`);
  return { ...result, reportSamples: rideTest.testRun.samples };
}

function firstMismatch(expected: unknown, actual: unknown, path: string): string | null {
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

function expectFormattedReadouts(
  actual: {
    readonly candidateHidden: boolean;
    readonly candidateLines: readonly string[];
    readonly hillHidden: boolean;
    readonly hillRows: readonly { readonly label: string; readonly value: string }[];
    readonly hillMessageHidden: boolean;
    readonly hillMessage: string;
  },
  estimate: ReturnType<typeof window.gameTest.constructionEstimates>,
): void {
  const selectedHill = estimate.selectedPiece?.hillEstimate ?? null;
  const candidateIsHidden = !!selectedHill || !estimate.candidateForcePreview;
  const candidateLines = selectedHill || !estimate.candidateForcePreview
    ? []
    : candidateForceReadout(estimate.candidateForcePreview, "metric");
  const formattedHill = selectedHill ? hillReadout(selectedHill, "metric") : { rows: [], message: "" };
  expect(actual).toEqual({
    candidateHidden: candidateIsHidden,
    candidateLines,
    hillHidden: formattedHill.rows.length === 0,
    hillRows: formattedHill.rows.map(({ label, value, note }) => ({
      label,
      value: note ? `${value} ${note}` : value,
    })),
    hillMessageHidden: formattedHill.message === "",
    hillMessage: formattedHill.message,
  });
}

function expectRideTargets(ride: RideFile, report: TestingReport): void {
  const targets = ride.expect;
  if (!targets) return;
  if (targets.excitement) expect(report.rating.excitement).toBeGreaterThanOrEqual(targets.excitement[0]);
  if (targets.excitement) expect(report.rating.excitement).toBeLessThanOrEqual(targets.excitement[1]);
  if (targets.sustainedMaxVerticalG) {
    // The CLI currently applies these ride-file targets to the report's maximum-force fields.
    expect(report.statistics.maxPositiveVerticalG).toBeGreaterThanOrEqual(targets.sustainedMaxVerticalG[0]);
    expect(report.statistics.maxPositiveVerticalG).toBeLessThanOrEqual(targets.sustainedMaxVerticalG[1]);
  }
  if (targets.sustainedMinVerticalG) {
    expect(report.statistics.maxNegativeVerticalG).toBeGreaterThanOrEqual(targets.sustainedMinVerticalG[0]);
    expect(report.statistics.maxNegativeVerticalG).toBeLessThanOrEqual(targets.sustainedMinVerticalG[1]);
  }
  if (targets.maxLateralG !== undefined) expect(report.statistics.maxLateralG).toBeLessThanOrEqual(targets.maxLateralG);
  if (targets.inversions !== undefined) expect(report.statistics.inversions).toBe(targets.inversions);
  if (targets.airtimeMomentsAtLeast !== undefined) expect(report.statistics.airtimeMoments).toBeGreaterThanOrEqual(targets.airtimeMomentsAtLeast);
}

async function reportCheckResults(
  testInfo: TestInfo,
  ride: RideFile,
  nativeCheck: NativeRideCheckWithSamples,
  report: TestingReport,
): Promise<void> {
  const check2Passed = nativeCheck.agreement.rows.filter(({ check2 }) => check2 === "Passed").length;
  const check3Passed = nativeCheck.agreement.rows.filter(({ check3 }) => check3 === "Passed").length;
  const browserCheck2 = browserCheck2Results(nativeCheck, report);
  const browserReportEnvelope = compareReportEnvelope(report, nativeCheck);
  const details = {
    ride: ride.name,
    check1: "gated by per-step wasm/native estimate and formatter comparisons",
    check2: {
      passedPieces: check2Passed,
      totalPieces: nativeCheck.agreement.rows.length,
      browserReport: browserCheck2,
    },
    check3: {
      passedPieces: check3Passed,
      totalPieces: nativeCheck.agreement.rows.length,
      perPieceWorstCarSource: "native agreement table; the browser report exposes ride-wide statistics only",
    },
    agreement: nativeCheck.agreement,
    browserReportEnvelope,
    expectationTargetSemantics: "The ride-file sustainedMaxVerticalG and sustainedMinVerticalG targets currently compare with maxPositiveVerticalG and maxNegativeVerticalG peaks, matching the native checker; no sustained-force target is verified.",
    expectationFailures: nativeCheck.expectationFailures,
  };
  console.info(`${ride.name}: check 2 native ${check2Passed}/${nativeCheck.agreement.rows.length} pieces; browser report ${browserCheck2.passedPieces}/${browserCheck2.totalPieces}; check 3 native ${check3Passed}/${nativeCheck.agreement.rows.length} pieces`);
  await testInfo.attach("ride-agreement-checks.json", {
    body: Buffer.from(JSON.stringify(details, null, 2)),
    contentType: "application/json",
  });
}

function browserCheck2Results(nativeCheck: NativeRideCheckWithSamples, report: TestingReport): {
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
    if (!pieceSamples?.length || !row.finishedTrackEstimate || row.entrySpeedEstimate === null) {
      failures.push(`piece ${row.pieceIndex} ${row.pieceName}: browser report samples or finished estimate unavailable`);
      failedPieces.add(row.pieceIndex);
      continue;
    }
    const measured: ForcePeaks = {
      minimumVerticalG: Math.min(...pieceSamples.map(({ verticalG }) => verticalG)),
      maximumVerticalG: Math.max(...pieceSamples.map(({ verticalG }) => verticalG)),
      maximumLateralG: Math.max(...pieceSamples.map(({ lateralG }) => Math.abs(lateralG))),
      maximumLongitudinalG: Math.max(...pieceSamples.map(({ longitudinalG }) => Math.abs(longitudinalG))),
    };
    const entrySpeedMeasured = pieceSamples[0].speed;
    if (Math.abs(row.entrySpeedEstimate - entrySpeedMeasured) > 0.5) {
      failures.push(`piece ${row.pieceIndex} entry speed: estimate ${row.entrySpeedEstimate.toFixed(3)} vs browser front car ${entrySpeedMeasured.toFixed(3)} m/s`);
      failedPieces.add(row.pieceIndex);
    }
    for (const axis of Object.keys(measured) as (keyof ForcePeaks)[]) {
      const difference = Math.abs(row.finishedTrackEstimate[axis] - measured[axis]);
      if (difference > 0.2) {
        failures.push(`piece ${row.pieceIndex} ${axis}: estimate differs from browser front-car report by ${difference.toFixed(3)} g`);
        failedPieces.add(row.pieceIndex);
      }
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

function compareReportEnvelope(report: TestingReport, nativeCheck: NativeRideCheck): {
  readonly browser: ForcePeaks;
  readonly nativeWorstCarEnvelope: ForcePeaks;
  readonly differences: ForcePeaks;
} {
  const browser: ForcePeaks = {
    minimumVerticalG: report.statistics.maxNegativeVerticalG,
    maximumVerticalG: report.statistics.maxPositiveVerticalG,
    maximumLateralG: report.statistics.maxLateralG,
    maximumLongitudinalG: Math.max(report.statistics.maxForwardG, report.statistics.maxBrakingG),
  };
  const worstCarRows = nativeCheck.agreement.rows.map(({ measuredWorst }) => measuredWorst).filter((values): values is ForcePeaks => values !== null);
  if (worstCarRows.length === 0) throw new Error("Native agreement report has no worst-car measurements");
  const nativeWorstCarEnvelope: ForcePeaks = {
    minimumVerticalG: Math.min(...worstCarRows.map(({ minimumVerticalG }) => minimumVerticalG)),
    maximumVerticalG: Math.max(...worstCarRows.map(({ maximumVerticalG }) => maximumVerticalG)),
    maximumLateralG: Math.max(...worstCarRows.map(({ maximumLateralG }) => maximumLateralG)),
    maximumLongitudinalG: Math.max(...worstCarRows.map(({ maximumLongitudinalG }) => maximumLongitudinalG)),
  };
  const differences = {
    minimumVerticalG: browser.minimumVerticalG - nativeWorstCarEnvelope.minimumVerticalG,
    maximumVerticalG: browser.maximumVerticalG - nativeWorstCarEnvelope.maximumVerticalG,
    maximumLateralG: browser.maximumLateralG - nativeWorstCarEnvelope.maximumLateralG,
    maximumLongitudinalG: browser.maximumLongitudinalG - nativeWorstCarEnvelope.maximumLongitudinalG,
  };
  return { browser, nativeWorstCarEnvelope, differences };
}

