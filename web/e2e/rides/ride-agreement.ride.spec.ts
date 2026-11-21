import { readdirSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { candidateForceReadout, hillReadout } from "../../src/candidate-force-readout";
import { shapeSelectionId } from "../../src/construction-shaped-elements";
import type { TestingReport } from "../../src/generated/TestingReport";
import type { CoasterModelKey } from "../../src/generated/coaster-models";
import { parseReplaySession } from "../../src/session-replay";
import { advanceUntilState, clickCanvas, openTool, placeStation, selectUnits, setGameSpeed, showBuilderTab, startPark, testRide } from "../player/driver";
import { placeCircuit, selectionFromRideIntent } from "../player/place-circuit";
import { expectRideReportExplained } from "../player/ride-report";
import { readRide, type RideFeatureStep, type RideFile, type RideStep } from "../player/rides";
import { runDesignJson } from "./design-json";
import { browserPathCommands, decisiveConstructionCommands } from "./fast-agreement";
import { browserAgreementResults, firstMismatch, type ForcePeaks, type NativeRideCheck, type NativeRideCheckWithSamples } from "./agreement-comparison";

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
    expectRideFeatures(ride, nativeCheck);
    const pageErrors: string[] = [];
    page.on("pageerror", (error) => pageErrors.push(`${error.message}\n${error.stack ?? "(no stack)"}`));

    if (stem === "velocity-viper") await page.setViewportSize({ width: 800, height: 480 });
    await startPark(page, {
      park: "new",
      seed: stem === "velocity-viper" ? 2026 : 149,
      speed: 1,
      livePauseRender: stem === "velocity-viper",
      testMode: true,
    });
    const stationHeight = await prepareStationTerrain(page, ride);
    await placeStation(page, {
      model: ride.model as CoasterModelKey,
      height: stationHeight,
      tile: { x: ride.station.x, y: ride.station.y, z: 2 },
      direction: stationDirection(ride.station.x, ride.station.y, ride.station.heading),
    });

    const expectedByStep = new Map(nativeCheck.browserEstimates.map((estimate) => [estimate.stepIndex, estimate]));
    let estimateComparisons = 0;
    const placementSteps = stem === "velocity-viper"
      ? ride.steps.filter((step) => !("close" in step))
      : ride.steps;
    await placeCircuit(page, placementSteps, {
      carsPerTrain: ride.carsPerTrain,
      onEstimate: async (phase, step, stepNumber) => {
        const stepName = "place" in step
          ? step.place
          : "intent" in step
            ? "hill" in step.intent
              ? `shaped hill ${step.intent.hill.feel}`
              : `shaped turn ${step.intent.turn.hand}`
            : `non-placement step ${stepNumber}`;
        if (!("place" in step) && !("intent" in step)) {
          throw new Error(`Estimate callback for non-placement step ${stepNumber}`);
        }
        const expected = expectedByStep.get(stepNumber);
        if (!expected) throw new Error(`Native estimate missing for ride step ${stepNumber} (${stepName})`);
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
          const shapePanel = builder.querySelector<HTMLElement>("[data-shape-readout]");
          const appendReason = builder.querySelector<HTMLElement>("[data-append-reason]");
          const appendButton = builder.querySelector<HTMLButtonElement>("[data-append]");
          if (!candidatePanel || !hillPanel || !hillMessage || !shapePanel || !appendReason || !appendButton) {
            throw new Error("Missing builder estimate or placement readouts");
          }
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
              candidateHidden: Boolean(candidatePanel.hidden),
              candidateLines: Array.from(candidatePanel.querySelectorAll("p"), (paragraph) => textOf(paragraph).trim()),
              hillHidden: Boolean(hillPanel.hidden),
              hillRows,
              hillMessageHidden: Boolean(hillMessage.hidden),
              hillMessage: textOf(hillMessage).trim(),
              shapeText: textOf(shapePanel).replace(/\s+/g, " ").trim(),
              appendDisabled: appendButton.disabled,
              appendReasonHidden: Boolean(appendReason.hidden),
              appendReason: textOf(appendReason).trim(),
            },
          };
        });
        const browserSnapshot = browserView.estimate;
        const browserWire = {
          selected: browserSnapshot.selected,
          selectedShape: browserSnapshot.selectedShape,
          selectedPieceId: browserSnapshot.selectedPiece?.id ?? null,
          constructionForcePreview: browserSnapshot.constructionForcePreview,
          candidateForcePreview: browserSnapshot.candidateForcePreview,
          speedMatch: browserSnapshot.speedMatch,
          hillEstimate: browserSnapshot.hillEstimate,
        };
        const expectedWire = {
          ...nativeSnapshot,
          selectedPieceId: nativeSnapshot.selected.startsWith("element:") || nativeSnapshot.selected.startsWith("shape:")
            ? null
            : nativeSnapshot.selected,
        };
        const mismatch = firstMismatch(expectedWire, browserWire, `${phase} estimate at ride step ${stepNumber}`);
        const candidateSampleContext = mismatch?.includes("candidateForcePreview.forceEnvelopeSamples[0]")
          ? `; native first envelope sample=${JSON.stringify(firstCandidateForceEnvelopeSample(nativeSnapshot.candidateForcePreview))}`
            + `; browser first envelope sample=${JSON.stringify(firstCandidateForceEnvelopeSample(browserSnapshot.candidateForcePreview))}`
            + `; selected shape digests=${JSON.stringify({
              native: nativeSnapshot.selectedShape?.spec?.geometryDigest ?? null,
              browser: browserSnapshot.selectedShape?.spec?.geometryDigest ?? null,
            })}`
          : "";
        expect(mismatch, `${phase} estimate at ride step ${stepNumber}${candidateSampleContext}`).toBeNull();
        if (phase === "selected" && "intent" in step) {
          const nativeGeometryDigest = nativeSnapshot.selectedShape?.spec?.geometryDigest;
          const browserGeometryDigest = browserSnapshot.selectedShape?.spec?.geometryDigest;
          expect(nativeGeometryDigest, `native shaped geometry digest at ride step ${stepNumber}`).toMatch(/^[0-9a-f]{16}$/);
          expect(browserGeometryDigest, `browser shaped geometry digest at ride step ${stepNumber}`).toBe(nativeGeometryDigest);
        }
        expectFormattedReadouts(browserView.readouts, browserSnapshot);
        if (stem === "horizon-hawk" && phase === "selected" && "intent" in step
          && "turn" in step.intent && step.intent.turn.size !== "Auto" && step.intent.turn.size.Radius === 68) {
          expect(browserView.readouts.shapeText).toContain("71.53° bank");
          expect(browserView.readouts.appendDisabled).toBe(false);
          expect(browserView.readouts.appendReasonHidden).toBe(true);
          expect(browserView.readouts.appendReason).toBe("");
        }
        estimateComparisons += 1;
      },
    });
    expect(estimateComparisons).toBe(nativeCheck.browserEstimates.length * 2);

    await showBuilderTab(page, "Build");
    const closeCircuitButton = page.getByRole("button", { name: "Close circuit" });
    const testCoasterButton = page.getByRole("button", { name: "Test coaster" });
    if (stem === "velocity-viper") {
      await expect(closeCircuitButton).toBeEnabled();
      await expect(testCoasterButton).toBeDisabled();
      await page.locator("canvas").focus();
      await page.keyboard.press("c");
      await expect(closeCircuitButton).toBeDisabled();
      await expect(testCoasterButton).toBeEnabled();
    } else {
      await expect(closeCircuitButton).toBeDisabled();
      await expect(testCoasterButton).toBeEnabled();
    }
    const crestBurstsBeforeTest = stem === "briar-blitz" || stem === "velocity-viper"
      ? await page.evaluate(() => window.gameTest.crestBursts().length)
      : null;
    if (stem === "velocity-viper") await setGameSpeed(page, 16);
    if (stem === "velocity-viper") {
      await openTool(page, "Coaster");
      await page.locator("canvas").focus();
      await page.keyboard.press("t");
    } else {
      await testRide(page);
    }
    await advanceUntilState(page, async () => !!(await page.evaluate(() => window.gameTest.testingReport())), { ticks: 40_000 });

    const report = await expectRideReportExplained(page);
    expectRideTargets(ride, report, nativeCheck);
    if (stem === "horizon-hawk") {
      expect(report.statistics.launches).toBe(0);
      expect(report.statistics.maxNegativeVerticalG).toBeGreaterThanOrEqual(-1.3);
    }
    if (stem === "daisy-dash") {
      expect(report.statistics.maxPositiveVerticalG).toBeLessThanOrEqual(3);
      expect(report.statistics.maxNegativeVerticalG).toBeGreaterThanOrEqual(-0.6);
      expect(report.statistics.maxLateralG).toBeLessThanOrEqual(1);
    }
    expect(report.notes.join(" ")).not.toMatch(/could not finish|rolled back|rated speed band/i);
    await expect(page.getByRole("region", { name: "Coaster construction" }).getByRole("status"))
      .toHaveText("Train completed a lap.");
    await expect(page.locator("[data-testing]")).toContainText("Scores (about 0–10)");
    if (stem === "briar-blitz" || stem === "velocity-viper") {
      if (crestBurstsBeforeTest === null) throw new Error(`${stem} did not record its initial crest bursts`);
      expect(await page.evaluate(() => window.gameTest.crestBursts().length)).toBeGreaterThan(crestBurstsBeforeTest);
    }
    if (stem === "timber-tumbler") {
      await expect(page.locator('[data-score-finding="airtime"] .testing-effect[data-score="excitement"]')).toHaveText(/^Excitement \+/);
      const maximumHeight = page.locator("[data-report-maximum-height]");
      const metricMaximumHeight = Number((await maximumHeight.textContent())?.match(/\d+(?:\.\d+)?/)?.[0]);
      expectNoPageErrors(pageErrors, "before Timber unit conversion");
      await selectUnits(page, "imperial");
      const imperialMaximumHeight = Number((await maximumHeight.textContent())?.match(/\d+(?:\.\d+)?/)?.[0]);
      expect(metricMaximumHeight).toBeCloseTo(imperialMaximumHeight * 0.3048, 0);
      expectNoPageErrors(pageErrors, "after Timber imperial unit toggle");
      await selectUnits(page, "metric");
      expectNoPageErrors(pageErrors, "after Timber metric unit toggle");

      expectNoPageErrors(pageErrors, "before Timber speed overlay selection");
      await page.getByLabel("Track overlay").selectOption("speed");
      expectNoPageErrors(pageErrors, "after Timber speed overlay selection");
      const overlaySize = await page.locator("canvas").evaluate((canvas: HTMLCanvasElement) => ({
        width: canvas.width,
        height: canvas.height,
      }));
      const overlayPick = await page.evaluate((size) => {
        const report = window.gameTest.testingReport();
        if (!report) throw new Error("Missing coaster test report");
        for (const sample of report.samples) {
          const point = window.gameTest.project(sample);
          if (point.x < 0 || point.x >= size.width || point.y < 0 || point.y >= size.height) continue;
          if (window.gameTest.pick(point.x, point.y)?.kind === "testing") return point;
        }
        throw new Error("No speed overlay segment was pickable");
      }, overlaySize);
      await clickCanvas(page, overlayPick);
      await expect(page.locator("[data-inspection]")).toContainText("Vertical:");
      expectNoPageErrors(pageErrors, "after Timber overlay inspection");

      if (process.env.RECORD_JOURNEY_SESSIONS === "1") {
        await expect(page.getByRole("button", { name: "Close circuit" })).toBeDisabled();
        await openTool(page, "Watch");
        const downloadPromise = page.waitForEvent("download");
        await page.getByRole("button", { name: "Export session", exact: true }).click();
        const download = await downloadPromise;
        expect(download.suggestedFilename()).toMatch(/^session-.*\.json$/);
        const recordingPath = await download.path();
        if (!recordingPath) throw new Error("The exported Timber journey session is unavailable");
        const session = parseReplaySession(JSON.parse(await readFile(recordingPath, "utf8")) as unknown);
        const sessionPath = fileURLToPath(new URL("../sessions/timber-tumbler-built.json", import.meta.url));
        await writeFile(sessionPath, `${JSON.stringify({ ...session, expect: { constructionClosed: true } }, null, 2)}\n`);
        await openTool(page, "Coaster");
      }
    }
    if (stem === "velocity-viper") await expectLaunchReport(page, report);
    await reportCheckResults(testInfo, ride, nativeCheck, report);

    if (stem === "velocity-viper") await expectVelocityViperTestingPanelScreenshot(page);
    expectNoPageErrors(pageErrors, `before ${ride.name} screenshot`);
    await page.screenshot({ path: testInfo.outputPath(`${stem}-ride.png`), fullPage: true });

    expectNoPageErrors(pageErrors, `after ${ride.name} screenshot`);
    await expectFastCheckCommands(page, stem);
  });
}

test("placeCircuit replays lift-fed shaped elements through visible controls @fast", async ({ page }) => {
  const ride = {
    name: "Horizon Hawk shaped replay fixture",
    model: "hyper",
    station: { x: 241, y: 249, z: 10, heading: "north" as const },
  };
  await startPark(page, { park: "new", seed: 156, speed: 1, testMode: true });
  const stationHeight = await prepareStationTerrain(page, ride);
  await placeStation(page, {
    model: ride.model as CoasterModelKey,
    height: stationHeight,
    tile: { x: ride.station.x, y: ride.station.y, z: 2 },
    direction: stationDirection(ride.station.x, ride.station.y, ride.station.heading),
  });

  const steps: RideStep[] = readRide("horizon-hawk").steps.slice(0, 12);
  const initialTrackCount = await page.locator("[data-track] li").count();
  let shapedEstimateCount = 0;
  await placeCircuit(page, steps, {
    carsPerTrain: 8,
    onEstimate: async (phase, step) => {
      if (!("intent" in step)) return;
      shapedEstimateCount += 1;
      const selected = shapeSelectionId(selectionFromRideIntent(step.intent));
      if (phase === "selected") {
        await expect(page.locator("[data-construction]")).toHaveAttribute("data-selected", selected);
        await expect(page.locator("[data-shape-readout]")).toContainText("Design speed:");
      } else {
        await expect(page.locator("[data-track] li").last()).toContainText("Shaped");
      }
    },
  });

  const placementCount = steps.filter((step) => "place" in step || "intent" in step).length;
  await expect(page.locator("[data-track] li")).toHaveCount(initialTrackCount + placementCount);
  expect(shapedEstimateCount).toBe(4);
});

/** The fast agreement check (tools/design agree) must send the builder commands this browser run sent. */
async function expectFastCheckCommands(page: Page, stem: string): Promise<void> {
  await openTool(page, "Watch");
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export session", exact: true }).click();
  const recordingPath = await (await downloadPromise).path();
  if (!recordingPath) throw new Error(`The exported ${stem} session is unavailable`);
  const session = parseReplaySession(JSON.parse(await readFile(recordingPath, "utf8")) as unknown);
  const browserCommands = decisiveConstructionCommands(session.commands.map(({ command }) => command));
  const fastCheckCommands = decisiveConstructionCommands(browserPathCommands(resolve(ridesDirectory, `${stem}.ride.json`)));
  expect(fastCheckCommands, `${stem} fast agreement check builder commands`).toEqual(browserCommands);
}

function expectNoPageErrors(pageErrors: readonly string[], checkpoint: string): void {
  expect(pageErrors, `Unexpected page errors ${checkpoint}`).toEqual([]);
}

async function prepareStationTerrain(page: Page, ride: Pick<RideFile, "model" | "name" | "station">): Promise<number> {
  await selectUnits(page, "metric");
  await openTool(page, "Coaster");
  const coasterModel = page.getByRole("combobox", { name: "Coaster model" });
  await coasterModel.selectOption(`model:${ride.model}`);

  const stationHeight = page.getByLabel("Station height above ground (metres)");
  const minimumAttribute = await stationHeight.getAttribute("min");
  if (minimumAttribute === null) throw new Error("Station height control has no minimum");
  const minimumStationHeight = Number(minimumAttribute);
  if (!Number.isFinite(minimumStationHeight)) throw new Error(`Invalid station height minimum: ${minimumAttribute}`);

  const stationHeightAboveGround = ride.station.z - newParkSurfaceHeight;
  if (stationHeightAboveGround >= minimumStationHeight) return stationHeightAboveGround;

  const adjustedSurfaceHeight = ride.station.z - minimumStationHeight;
  if (!Number.isInteger(adjustedSurfaceHeight) || adjustedSurfaceHeight < 0) {
    throw new Error(`Cannot lower terrain to place ${ride.name} at absolute station height ${ride.station.z}`);
  }
  await page.evaluate(({ x, y, height }) => {
    window.gameTest.setSurface(x, y, [height, height, height, height]);
  }, { x: ride.station.x, y: ride.station.y, height: adjustedSurfaceHeight });
  await page.evaluate(() => window.gameTest.advanceTicks(0));
  return minimumStationHeight;
}

async function expectLaunchReport(page: Page, report: TestingReport): Promise<void> {
  const launchPositions = page.getByRole("region", { name: "Launch positions" }).locator("li");
  await expect(launchPositions).toHaveCount(4);
  const launchCentres = (await launchPositions.allTextContents()).map((text) => {
    const match = /(\d+(?:\.\d+)?) m from station/.exec(text);
    if (!match) throw new Error(`Launch position has no distance: ${text}`);
    return Number(match[1]);
  });
  const launchSpeeds = launchCentres.map((centre) => report.samples.reduce((closest, sample) =>
    Math.abs(sample.distance - centre) < Math.abs(closest.distance - centre) ? sample : closest).speed);
  expect(launchSpeeds).toHaveLength(4);
  for (const speed of launchSpeeds) expect(speed).toBeGreaterThan(2);
}

async function expectVelocityViperTestingPanelScreenshot(page: Page): Promise<void> {
  await page.locator("canvas").focus();
  await page.keyboard.press("Space");
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
  await page.setViewportSize({ width: 1280, height: 1200 });
  const testingPanel = page.locator("[data-testing]");
  await page.evaluate(() => {
    const dock = document.querySelector<HTMLElement>("[data-dock]");
    const testing = document.querySelector<HTMLElement>("[data-testing]");
    if (!dock || !testing) throw new Error("Coaster testing panel is missing");
    dock.scrollTop += testing.getBoundingClientRect().top - dock.getBoundingClientRect().top;
  });
  const panelHeight = await testingPanel.evaluate((element) => Math.ceil(element.getBoundingClientRect().height));
  await page.setViewportSize({ width: 1280, height: Math.max(720, panelHeight + 100) });
  await expect(testingPanel).toHaveScreenshot("coaster-testing-panel.png", { animations: "disabled" });
  await page.setViewportSize({ width: 1280, height: 720 });
}

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
  const result = runDesignJson<NativeRideCheckWithSamples[]>(resolve(repositoryRoot, "tools/design"), [
    "check",
    "--json",
    ridePath,
  ], repositoryRoot)[0];
  if (!result) throw new Error(`Native design check omitted ${stem}`);
  if (!result.reportSamples?.length) throw new Error(`Native test report samples omitted ${stem}`);
  return result;
}

function firstCandidateForceEnvelopeSample(preview: unknown): unknown {
  if (preview === null || typeof preview !== "object" || !("forceEnvelopeSamples" in preview)) return null;
  const samples = preview.forceEnvelopeSamples;
  return Array.isArray(samples) ? samples[0] : null;
}

function expectFormattedReadouts(
  actual: {
    readonly candidateHidden: boolean;
    readonly candidateLines: readonly string[];
    readonly hillHidden: boolean;
    readonly hillRows: readonly { readonly label: string; readonly value: string }[];
    readonly hillMessageHidden: boolean;
    readonly hillMessage: string;
    readonly shapeText: string;
    readonly appendDisabled: boolean;
    readonly appendReasonHidden: boolean;
    readonly appendReason: string;
  },
  estimate: ReturnType<typeof window.gameTest.constructionEstimates>,
): void {
  const selectedHill = estimate.selectedPiece?.hillEstimate ?? null;
  const candidateIsHidden = !!selectedHill || !estimate.candidateForcePreview;
  const candidateLines = selectedHill || !estimate.candidateForcePreview
    ? []
    : candidateForceReadout(estimate.candidateForcePreview, "metric");
  const formattedHill = selectedHill ? hillReadout(selectedHill, "metric") : { rows: [], message: "" };
  const {
    appendDisabled,
    appendReasonHidden,
    appendReason,
    ...readouts
  } = actual;
  expect(readouts).toEqual({
    candidateHidden: candidateIsHidden,
    candidateLines,
    hillHidden: formattedHill.rows.length === 0,
    hillRows: formattedHill.rows.map(({ label, value, note }) => ({
      label,
      value: note ? `${value} ${note}` : value,
    })),
    hillMessageHidden: formattedHill.message === "",
    hillMessage: formattedHill.message,
    shapeText: estimate.selectedShape ? expect.stringContaining("Design speed:") : "",
  });

  const refusal = estimate.selectedPiece
    ? estimate.selectedPiece.refusal
    : estimate.selectedShape?.refusal ?? "";
  expect({ appendDisabled, appendReasonHidden, appendReason }).toEqual({
    appendDisabled: refusal !== "",
    appendReasonHidden: refusal === "",
    appendReason: refusal,
  });
}

function nativeSustainedVerticalG(nativeCheck: NativeRideCheck, metric: string): number {
  const expectation = nativeCheck.expectations.find((candidate) => candidate.metric === metric);
  if (!expectation) throw new Error(`Native check omitted the ${metric} expectation`);
  if (typeof expectation.observed !== "number") throw new Error(`Native ${metric} has no sustained observation`);
  return expectation.observed;
}

function expectRideTargets(ride: RideFile, report: TestingReport, nativeCheck: NativeRideCheck): void {
  const targets = ride.expect;
  if (targets?.excitement) expect(report.rating.excitement).toBeGreaterThanOrEqual(targets.excitement[0]);
  if (targets?.excitement) expect(report.rating.excitement).toBeLessThanOrEqual(targets.excitement[1]);
  // The sustained targets are checked against the native checker's sustained force peaks (forces held over
  // a time window, ignoring brief spikes). The browser report has no sustained statistic, but its samples
  // must match the native samples within 1e-6, which reportCheckResults asserts.
  if (targets?.sustainedMaxVerticalG) {
    const sustainedMaximum = nativeSustainedVerticalG(nativeCheck, "maximum vertical g");
    expect(sustainedMaximum).toBeGreaterThanOrEqual(targets.sustainedMaxVerticalG[0]);
    expect(sustainedMaximum).toBeLessThanOrEqual(targets.sustainedMaxVerticalG[1]);
  }
  if (targets?.sustainedMinVerticalG) {
    const sustainedMinimum = nativeSustainedVerticalG(nativeCheck, "sustained minimum vertical g");
    expect(sustainedMinimum).toBeGreaterThanOrEqual(targets.sustainedMinVerticalG[0]);
    expect(sustainedMinimum).toBeLessThanOrEqual(targets.sustainedMinVerticalG[1]);
  }
  if (targets?.maxLateralG !== undefined) expect(report.statistics.maxLateralG).toBeLessThanOrEqual(targets.maxLateralG);
  if (targets?.inversions !== undefined) expect(report.statistics.inversions).toBe(targets.inversions);
  if (targets?.airtimeMomentsAtLeast !== undefined) {
    expect(report.statistics.airtimeMoments).toBeGreaterThanOrEqual(targets.airtimeMomentsAtLeast);
  }
  if (targets?.withinComfortableForceLimits !== undefined) {
    const allSamplesWithinLimits = report.samples.every((sample) => sample.forceLimitExceeded.every((exceeded) => !exceeded));
    expect(allSamplesWithinLimits).toBe(targets.withinComfortableForceLimits);
  }
  if (targets?.inversionsAtLeast !== undefined) {
    expect(report.statistics.inversions).toBeGreaterThanOrEqual(targets.inversionsAtLeast);
  }
  if (targets?.launches !== undefined) expect(report.statistics.launches).toBe(targets.launches);
  if (targets?.biggestDropAtLeast !== undefined) {
    expect(report.statistics.biggestDrop).toBeGreaterThanOrEqual(targets.biggestDropAtLeast);
  }
  if (targets?.maxSpeedAtLeast !== undefined) {
    expect(report.statistics.maxSpeed).toBeGreaterThanOrEqual(targets.maxSpeedAtLeast);
  }
  if (targets?.invertedSeconds !== undefined) {
    expect(report.statistics.invertedSeconds).toBeCloseTo(targets.invertedSeconds, 4);
  }
}

function expectRideFeatures(ride: RideFile, nativeCheck: NativeRideCheck): void {
  const features = ride.expect?.features;
  if (!features) return;

  const piecePrefixes = Object.entries(features.piecePrefixesAtLeast ?? {});
  const prefixGroups = features.piecePrefixGroupsAtLeast ?? [];
  const piecePrefixesAfter = features.piecePrefixesAfter ?? [];
  const sequences = [...(features.orderedSteps ?? []), ...(features.adjacentSteps ?? [])];
  const invalidPiecePrefix = piecePrefixes.some(([prefix, minimumCount]) =>
    prefix.length === 0 || !Number.isInteger(minimumCount) || minimumCount < 1);
  const invalidPrefixGroup = prefixGroups.some((group) => group.prefixes.length === 0
    || group.prefixes.some((prefix) => prefix.length === 0)
    || !Number.isInteger(group.minimum)
    || group.minimum < 1);
  const invalidPiecePrefixesAfter = piecePrefixesAfter.some(({ prefix, after }) =>
    prefix.length === 0
    || ("place" in after && after.place.length === 0)
    || ("placePrefix" in after && after.placePrefix.length === 0));
  const invalidHeartlinedInversionCount = features.heartlinedInversionsAtLeast !== undefined
    && (!Number.isInteger(features.heartlinedInversionsAtLeast) || features.heartlinedInversionsAtLeast < 1);
  const invalidSequence = sequences.some((sequence) => sequence.length === 0
    || sequence.some((step) => ("place" in step && step.place.length === 0)
      || ("placePrefix" in step && step.placePrefix.length === 0)));
  if (invalidPiecePrefix || invalidPrefixGroup || invalidPiecePrefixesAfter
    || invalidHeartlinedInversionCount || invalidSequence) {
    throw new Error(`${ride.name} has an empty or invalid feature expectation`);
  }

  if (features.heartlinedInversionsAtLeast !== undefined) {
    const minimumCount = features.heartlinedInversionsAtLeast;
    if (!Number.isInteger(minimumCount) || minimumCount < 1) {
      throw new Error(`${ride.name} has an invalid heartlined inversion feature expectation`);
    }
    const nativeExpectation = nativeCheck.expectations.find(
      (expectation) => expectation.metric === "heartlined inversion pieces",
    );
    if (!nativeExpectation || typeof nativeExpectation.observed !== "number") {
      throw new Error(`${ride.name} native check omitted its heartlined inversion count`);
    }
    expect(nativeExpectation.passed).toBe(true);
    expect(nativeExpectation.observed).toBeGreaterThanOrEqual(minimumCount);
  }

  for (const [piecePrefix, minimumCount] of Object.entries(features.piecePrefixesAtLeast ?? {})) {
    const observedCount = ride.steps.filter((step) => rideStepPieceId(step)?.startsWith(piecePrefix) === true).length;
    expect(observedCount, `${ride.name} needs at least ${minimumCount} pieces starting with ${piecePrefix}`)
      .toBeGreaterThanOrEqual(minimumCount);
  }
  for (const group of prefixGroups) {
    const observedCount = ride.steps.filter((step) => {
      const pieceId = rideStepPieceId(step);
      return pieceId !== undefined && group.prefixes.some((prefix) => pieceId.startsWith(prefix));
    }).length;
    expect(observedCount, `${ride.name} needs at least ${group.minimum} pieces matching ${group.prefixes.join(" or ")}`)
      .toBeGreaterThanOrEqual(group.minimum);
  }
  for (const { prefix, after } of piecePrefixesAfter) {
    const afterIndex = ride.steps.findIndex((step) => rideFeatureMatches(step, after));
    expect(afterIndex, `${ride.name} must contain the feature after ${describeRideFeatureSequence([after])}`)
      .toBeGreaterThanOrEqual(0);
    const matchingBeforeAnchor = ride.steps.slice(0, afterIndex).some((step) =>
      rideStepPieceId(step)?.startsWith(prefix) === true);
    expect(matchingBeforeAnchor, `${ride.name} places every ${prefix} after ${describeRideFeatureSequence([after])}`)
      .toBe(false);
  }
  for (const sequence of features.orderedSteps ?? []) {
    expect(
      containsOrderedRideFeatures(ride.steps, sequence),
      `${ride.name} must contain the ordered feature sequence ${describeRideFeatureSequence(sequence)}`,
    ).toBe(true);
  }
  for (const sequence of features.adjacentSteps ?? []) {
    expect(
      containsAdjacentRideFeatures(ride.steps, sequence),
      `${ride.name} must contain adjacent feature steps ${describeRideFeatureSequence(sequence)}`,
    ).toBe(true);
  }
}

function containsOrderedRideFeatures(
  rideSteps: readonly RideStep[],
  expectedSteps: readonly RideFeatureStep[],
): boolean {
  let nextRideStepIndex = 0;
  for (const expectedStep of expectedSteps) {
    while (nextRideStepIndex < rideSteps.length && !rideFeatureMatches(rideSteps[nextRideStepIndex], expectedStep)) {
      nextRideStepIndex += 1;
    }
    if (nextRideStepIndex === rideSteps.length) return false;
    nextRideStepIndex += 1;
  }
  return true;
}

function containsAdjacentRideFeatures(
  rideSteps: readonly RideStep[],
  expectedSteps: readonly RideFeatureStep[],
): boolean {
  return rideSteps.some((_, startIndex) => expectedSteps.every((expectedStep, offset) =>
    rideFeatureMatches(rideSteps[startIndex + offset], expectedStep)));
}

function rideFeatureMatches(rideStep: RideStep | undefined, expectedStep: RideFeatureStep): boolean {
  if (!rideStep) return false;
  const pieceId = rideStepPieceId(rideStep);
  if ("place" in expectedStep) return pieceId === expectedStep.place;
  if ("placePrefix" in expectedStep) return pieceId?.startsWith(expectedStep.placePrefix) ?? false;
  return "chain" in rideStep && rideStep.chain === expectedStep.chain;
}

function rideStepPieceId(rideStep: RideStep): string | undefined {
  if ("place" in rideStep) return rideStep.place;
  if (!("intent" in rideStep)) return undefined;

  switch (rideStep.spec.kind) {
    case "hill":
      return "ShapedHill";
    case "turn":
      if (!rideStep.spec.hand) throw new Error("Resolved shaped turn has no hand");
      return rideStep.spec.hand === "Left" ? "ShapedTurnLeft" : "ShapedTurnRight";
  }
}

function describeRideFeatureSequence(sequence: readonly RideFeatureStep[]): string {
  return sequence.map((step) => {
    if ("place" in step) return `place ${step.place}`;
    if ("placePrefix" in step) return `place starting with ${step.placePrefix}`;
    return `chain ${step.chain}`;
  }).join(" then ");
}

async function reportCheckResults(
  testInfo: TestInfo,
  ride: RideFile,
  nativeCheck: NativeRideCheckWithSamples,
  report: TestingReport,
): Promise<void> {
  const check2Passed = nativeCheck.agreement.rows.filter(({ check2 }) => check2 === "Passed").length;
  const check3Failures = nativeCheck.agreement.failures.filter(({ check }) => check === "PlacementEstimateVsWorstCar");
  const downstreamTrackFailures = check3Failures.filter(({ classification }) => classification === "downstream track").length;
  const estimateErrorFailures = check3Failures.filter(({ classification }) => classification === "estimate error").length;
  const unclassifiedPlacementFailures = check3Failures
    .filter(({ classification }) => classification !== "downstream track" && classification !== "estimate error")
    .map(({ pieceIndex, pieceName, metric }) => `piece ${pieceIndex} ${pieceName} ${metric}`);
  const browserAgreement = browserAgreementResults(nativeCheck, report);
  const browserReportEnvelope = compareReportEnvelope(report, nativeCheck);
  const details = {
    ride: ride.name,
    check1: "gated by per-step wasm/native estimate and formatter comparisons",
    check2: {
      passedPieces: check2Passed,
      totalPieces: nativeCheck.agreement.rows.length,
      browserReport: browserAgreement,
    },
    check3: {
      failureMessages: check3Failures.length,
      affectedPieces: new Set(check3Failures.map(({ pieceIndex }) => pieceIndex)).size,
      downstreamTrackFailures,
      estimateErrorFailures,
      unclassifiedFailures: unclassifiedPlacementFailures,
      failures: check3Failures,
      perPieceWorstCarSource: "browser test report peaks, checked against the native per-piece agreement table",
      gate: "informational; downstream track is expected and estimate errors are reported for correction",
    },
    agreement: nativeCheck.agreement,
    browserReportEnvelope,
    expectationTargetSemantics: "The ride-file sustainedMaxVerticalG and sustainedMinVerticalG targets compare with the native checker's sustained vertical force peaks; browser report samples must match the native samples.",
    expectationFailures: nativeCheck.expectationFailures,
  };
  console.info(`${ride.name}: check 2 native ${check2Passed}/${nativeCheck.agreement.rows.length} pieces; browser report parity ${browserAgreement.passedPieces}/${browserAgreement.totalPieces}; check 3 placement failures ${check3Failures.length} (${downstreamTrackFailures} downstream track, ${estimateErrorFailures} estimate error)`);
  await testInfo.attach("ride-agreement-checks.json", {
    body: Buffer.from(JSON.stringify(details, null, 2)),
    contentType: "application/json",
  });
  expect(nativeCheck.agreement.passed, `${ride.name} native agreement`).toBe(true);
  expect(browserAgreement.failures, `${ride.name} browser agreement measurements`).toEqual([]);
  expect(unclassifiedPlacementFailures, `${ride.name} classified placement failures`).toEqual([]);
  expect(check2Passed, `${ride.name} check 2 passing pieces`).toBe(nativeCheck.agreement.rows.length);
  expect(browserAgreement.passedPieces, `${ride.name} browser agreement pieces`).toBe(nativeCheck.agreement.rows.length);
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

