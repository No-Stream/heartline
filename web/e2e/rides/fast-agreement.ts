import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { initSync, BuilderSessionBridge, WasmSimulation } from "../../src/wasm/theme_park_sim_wasm.js";
import { shouldInvalidateConstructionFinishSearch } from "../../src/construction-finish-search.ts";
import { shapedElementSelection, shapeSelectionId, steppedShapedSelection, type ShapedElementSelection } from "../../src/construction-shaped-elements.ts";
import { readConstructionEstimates, type ConstructionEstimates } from "../../src/game-test.ts";
import { displaySpeedToMetresPerSecond, metresPerSecondToDisplay } from "../../src/units.ts";
import { applyConstructionAction, readBuilderEstimates, sendsIncompleteConstructionState, type BuilderWorkerEstimates } from "../../src/worker-construction.ts";
import type { ConstructionAction, ConstructionTrack } from "../../src/construction-protocol.ts";
import type { TestingReport } from "../../src/generated/TestingReport.ts";
import { defaultHillFeel, hillStepperOptions, hillToOpen, steppedHillSelection } from "../../src/construction-choices.ts";
import { classicHillTargets, hillAttributeOrder, hillFeelOrder, hillTwistOrder, selectionFromRideIntent, shapeBaseSelectionId, type ClassicHillAttribute } from "../player/ride-shape-selection.ts";
import { readRideFile, type RideFile, type RideHeading, type RideShapeStep, type RideStep } from "../player/rides.ts";
import { browserAgreementResults, firstMismatch, type NativeRideCheck, type NativeRideCheckWithSamples } from "./agreement-comparison.ts";

/**
 * Fast stand-in for the browser ride-agreement test (ride-agreement.ride.spec.ts). It replays a ride file in Node
 * through the same release WASM build, the worker's own command and estimate code (src/worker-construction.ts), and
 * the commands the visible builder controls send for each ride step (place-circuit.ts), then applies the browser
 * test's comparisons against the native design check with the same tolerances.
 */

const repositoryRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const wasmPath = resolve(repositoryRoot, "web/src/wasm/theme_park_sim_wasm_bg.wasm");
const browserTestTickLimit = 40_000;
const browserTestTickChunk = 500;
const blockAnalysisBudget = 1000;

export interface RideAgreementResult {
  readonly ride: string;
  readonly estimateComparisons: number;
  readonly failures: readonly string[];
  readonly seconds: { readonly wasm: number; readonly total: number };
}

let wasmReady = false;
function ensureWasm(): void {
  if (wasmReady) return;
  initSync({ module: readFileSync(wasmPath) });
  wasmReady = true;
}

type BuilderState = BuilderWorkerEstimates & { readonly selected: string | null; readonly chain: boolean };

/** One browser worker as the ride-agreement test drives it: the park, the builder session, and the last complete state. */
class WorkerReplay {
  readonly simulation: WasmSimulation;
  readonly builderSession = new BuilderSessionBridge();
  readonly commands: ConstructionAction[] = [];
  state: BuilderState | null = null;
  /** construction.ts opens the hill inspector on an ejector until the player has had any hill selected. */
  hillChosenBefore = false;

  constructor(seed: number) {
    // The ride-agreement test starts each ride with `/?park=new&seed=…&test=1`, which uses 12 guests.
    this.simulation = new WasmSimulation(BigInt(seed), 12, "new");
  }

  dispatch(action: ConstructionAction): void {
    this.commands.push(action);
    // The worker then runs the finish search in idle slices. Its result only decides whether Finish is offered
    // (construction_completion_* fields), which neither the estimates nor the test read, so it is not run here.
    if (shouldInvalidateConstructionFinishSearch(action)) this.simulation.construction_finish_invalidate();
    if (!applyConstructionAction(this.simulation, this.builderSession, action, () => {})) return;
    if (sendsIncompleteConstructionState(action, this.simulation)) this.readState(false);
    this.state = this.readState(true);
    while (!this.simulation.construction_advance_block_analysis(blockAnalysisBudget)) { /* the worker's scheduled analysis */ }
  }

  private readState(complete: boolean): BuilderState {
    const estimates = readBuilderEstimates(this.simulation, this.builderSession, complete);
    const selected = this.simulation.construction_selected() || null;
    if (estimates.hillOptions.some(({ id }) => id === selected)) this.hillChosenBefore = true;
    return { ...estimates, selected, chain: this.simulation.construction_chain() };
  }

  requireState(): BuilderState {
    if (!this.state) throw new Error("The builder has not sent a construction state yet");
    return this.state;
  }

  estimates(): ConstructionEstimates {
    return readConstructionEstimates(this.requireState());
  }

  track(): ConstructionTrack[] {
    return JSON.parse(this.simulation.construction_track()) as ConstructionTrack[];
  }
}

/** A step the browser driver (place-circuit.ts) could not perform through the visible controls; it ends the replay. */
class BrowserDriverRefusal extends Error {}

const headingIndex: Record<RideHeading, number> = { east: 0, north: 1, west: 2, south: 3 };

function select(replay: WorkerReplay, piece: string, exact: boolean): void {
  replay.dispatch(exact ? { type: "construction-select", piece, exact: true } : { type: "construction-select", piece });
}

/** Mirrors place-circuit.ts selectPiece for a classic piece or track element. */
function selectClassicPiece(replay: WorkerReplay, piece: string): void {
  const state = replay.requireState();
  if (state.selected === piece) return;
  if (piece.startsWith("element:")) {
    select(replay, piece, false);
  } else if (classicHillTargets(piece)) {
    selectHillInInspector(replay, classicHillTargets(piece)!);
  } else if (/^SpeedTurnLarge\d+(Left|Right)$/.test(piece)) {
    // The browser reaches these through the turn and bank steppers, ending on an exact selection of the piece.
    select(replay, piece, true);
  } else {
    // A palette click selects exactly only for fixed airtime hills.
    select(replay, piece, state.hillOptions.some(({ id }) => id === piece));
  }
  const selected = replay.requireState().selected;
  if (selected !== piece) throw new BrowserDriverRefusal(`the builder selected ${selected} for ${piece}`);
}

/** Mirrors place-circuit.ts: open the hill inspector, then step feel, twist, height and span to their targets. */
function selectHillInInspector(replay: WorkerReplay, targets: Readonly<Record<ClassicHillAttribute, string>>): void {
  const opening = replay.requireState();
  // Opening the section selects a hill unless a hill or shaped element is already selected (construction.ts toggle handler).
  if (!opening.selectedShape && !opening.hillOptions.some(({ id }) => id === opening.selected)) {
    const hill = hillToOpen(opening.hillOptions, opening.pieces, replay.hillChosenBefore ? null : defaultHillFeel);
    if (hill) select(replay, hill, false);
  }
  for (const attribute of hillAttributeOrder) stepHillTo(replay, attribute, targets[attribute]);
}

/** Mirrors place-circuit.ts stepHill with the page in metric units. */
function stepHillTo(replay: WorkerReplay, attribute: ClassicHillAttribute, target: string): void {
  for (let step = 0; step < 12; step++) {
    const state = replay.requireState();
    const options = hillStepperOptions(state.hillOptions, state.selected, attribute, "metric");
    if (!options) throw new BrowserDriverRefusal(`No hill is selected to step its ${attribute} to ${target}`);
    const current = options.values[options.index];
    if (current === target) return;
    const order = attribute === "feel" ? hillFeelOrder : hillTwistOrder;
    const isSize = attribute === "height" || attribute === "span";
    const currentRank = isSize ? Number.parseFloat(current) : order.indexOf(current);
    const targetRank = isSize ? Number.parseFloat(target) : order.indexOf(target);
    if (!Number.isFinite(currentRank) || currentRank < 0 || targetRank < 0) throw new BrowserDriverRefusal(`Cannot select ${attribute} ${target} from ${current}`);
    const action = steppedHillSelection(state.hillOptions, state.selected, attribute, currentRank < targetRank ? 1 : -1);
    if (!action) throw new BrowserDriverRefusal(`Cannot select ${attribute} ${target} from ${current}: the stepper is at its end`);
    replay.dispatch(action);
    const after = replay.requireState();
    const stepped = hillStepperOptions(after.hillOptions, after.selected, attribute, "metric");
    if (stepped?.values[stepped.index] === current) throw new BrowserDriverRefusal(`The ${attribute} stepper did not move from ${current}`);
  }
  throw new BrowserDriverRefusal(`Could not select hill ${attribute} ${target}`);
}

function stepShape(replay: WorkerReplay, attribute: "strength" | "size" | "radius", step: -1 | 1): void {
  const shape = replay.requireState().selectedShape;
  if (!shape) throw new BrowserDriverRefusal(`No shaped element is selected to adjust its ${attribute}`);
  const selection = steppedShapedSelection(shape, attribute, step);
  if (!selection) throw new BrowserDriverRefusal(`The ${attribute} stepper cannot adjust ${shape.selection}`);
  select(replay, shapeSelectionId(selection), true);
}

function resolvedShapeSize(replay: WorkerReplay, kind: "hill" | "turn"): number {
  const spec = replay.requireState().selectedShape?.spec;
  const size = kind === "hill" ? spec?.heightM : spec?.radiusM;
  if (size === null || size === undefined) throw new BrowserDriverRefusal("Selected shaped element has no resolved size");
  return size;
}

/** Mirrors place-circuit.ts setFixedShapeDimension. */
function setFixedShapeDimension(replay: WorkerReplay, kind: "hill" | "turn", targetMetres: number): void {
  if (Math.abs(targetMetres / 2 - Math.round(targetMetres / 2)) > 1e-8) {
    throw new BrowserDriverRefusal(`The visible stepper only selects 2 m increments; ride intent requests ${targetMetres} m`);
  }
  const targetGridValue = Math.round(targetMetres / 2) * 2;
  let currentGridValue = Math.round(resolvedShapeSize(replay, kind) / 2) * 2;
  let selectedFixedSize = false;
  while (currentGridValue !== targetGridValue || !selectedFixedSize) {
    const before = currentGridValue;
    const fixedBeforeClick = selectedFixedSize;
    stepShape(replay, kind === "hill" ? "size" : "radius", currentGridValue < targetGridValue ? 1 : -1);
    selectedFixedSize = true;
    currentGridValue = Math.round(resolvedShapeSize(replay, kind) / 2) * 2;
    if (fixedBeforeClick && currentGridValue !== targetGridValue && Math.abs(currentGridValue - targetGridValue) >= Math.abs(before - targetGridValue)) {
      throw new BrowserDriverRefusal(`The visible ${kind} size stepper cannot reach ${targetMetres} m: one click from ${before} m resolves to ${currentGridValue} m`);
    }
  }
}

/** Mirrors place-circuit.ts selectShapedStep. */
function selectShapedStep(replay: WorkerReplay, step: RideShapeStep): string {
  const intentSelection: ShapedElementSelection = selectionFromRideIntent(step.intent);
  const baseId = shapeBaseSelectionId(intentSelection);
  if (intentSelection.kind === "hill") {
    select(replay, baseId, false);
    const strengthOf = (): number => {
      const shape = replay.requireState().selectedShape;
      if (!shape) throw new BrowserDriverRefusal(`Shaped hill ${baseId} did not select a shaped element`);
      const selection = shapedElementSelection(shape);
      if (selection.kind !== "hill") throw new BrowserDriverRefusal(`Shaped hill ${baseId} selected a shaped ${selection.kind}`);
      return selection.strengthG;
    };
    const defaultId = shapeSelectionId({ ...intentSelection, strengthG: strengthOf(), size: { kind: "auto" } });
    if (replay.requireState().selected !== defaultId) {
      throw new BrowserDriverRefusal(`Shaped hill palette choice selected ${replay.requireState().selected}, expected ${defaultId}`);
    }
    const targetStrength = intentSelection.strengthG;
    const strengthSteps = (strengthOf() - targetStrength) / 0.1;
    if (Math.abs(strengthSteps - Math.round(strengthSteps)) > 1e-5) {
      throw new BrowserDriverRefusal(`Visible strength stepper cannot select ${targetStrength} g from ${strengthOf()} g in 0.1 g steps`);
    }
    while (Math.abs(strengthOf() - targetStrength) > 1e-5) {
      const direction = strengthOf() > targetStrength ? 1 : -1;
      const expectedStrength = strengthOf() - direction * 0.1;
      stepShape(replay, "strength", direction);
      // place-circuit.ts asserts each click moves exactly 0.1 g (toBeCloseTo with 5 digits).
      if (!(Math.abs(strengthOf() - expectedStrength) < 5e-6)) {
        throw new BrowserDriverRefusal(`Strength stepper must move to ${expectedStrength} g while selecting ${targetStrength} g; it selected ${strengthOf()} g`);
      }
    }
    if (intentSelection.size.kind === "height") setFixedShapeDimension(replay, "hill", intentSelection.size.heightMetres);
  } else {
    if (replay.requireState().selected !== baseId) select(replay, baseId, false);
    if (intentSelection.size.kind === "radius") setFixedShapeDimension(replay, "turn", intentSelection.size.radiusMetres);
  }
  const expectedId = shapeSelectionId(intentSelection);
  if (replay.requireState().selected !== expectedId) {
    throw new BrowserDriverRefusal(`Visible shaped-element controls selected ${replay.requireState().selected}, but ride intent requires ${expectedId}`);
  }
  return expectedId;
}

/** Mirrors place-circuit.ts setLaunchSpeed: the player types the metric display speed into the last launch's input. */
function setLaunchSpeed(replay: WorkerReplay, launchSpeed: number): void {
  const launch = replay.track().at(-1);
  if (!launch || launch.launchMinimumSpeed === null || launch.launchMaximumSpeed === null) {
    throw new BrowserDriverRefusal("Placed launch piece has no target speed control");
  }
  const minimumDisplaySpeed = metresPerSecondToDisplay(launch.launchMinimumSpeed, "metric");
  const maximumDisplaySpeed = metresPerSecondToDisplay(launch.launchMaximumSpeed, "metric");
  const displaySpeed = Number(String(metresPerSecondToDisplay(launchSpeed, "metric")));
  if (displaySpeed < minimumDisplaySpeed || displaySpeed > maximumDisplaySpeed) {
    throw new BrowserDriverRefusal(`Ride launch speed ${launchSpeed} m/s is outside the builder bounds ${minimumDisplaySpeed}–${maximumDisplaySpeed} km/h`);
  }
  const clampedSpeed = Math.max(minimumDisplaySpeed, Math.min(maximumDisplaySpeed, displaySpeed));
  replay.dispatch({ type: "construction-set-launch-intensity", id: launch.id, targetSpeed: displaySpeedToMetresPerSecond(clampedSpeed, "metric") });
  const stored = replay.track().at(-1)?.launchTargetSpeed;
  if (stored !== launchSpeed) throw new BrowserDriverRefusal(`Launch target speed is ${stored} m/s after typing ${launchSpeed} m/s`);
}

function setCarsPerTrain(replay: WorkerReplay, carsPerTrain: number): void {
  const carsNow = (): number => (JSON.parse(replay.simulation.construction_train_layout()) as { carsPerTrain: number }).carsPerTrain;
  while (carsNow() !== carsPerTrain) {
    const current = carsNow();
    replay.dispatch({ type: "construction-set-cars-per-train", cars: carsPerTrain < current ? current - 1 : current + 1 });
    if (carsNow() === current) throw new BrowserDriverRefusal(`Cannot set cars per train from ${current} to ${carsPerTrain}`);
  }
}

function compareEstimate(
  failures: string[],
  phase: "selected" | "placed",
  stepNumber: number,
  isShaped: boolean,
  nativeSnapshot: NativeRideCheck["browserEstimates"][number]["selection"],
  browserSnapshot: ConstructionEstimates,
): void {
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
    selectedPieceId: nativeSnapshot.selected.startsWith("element:") || nativeSnapshot.selected.startsWith("shape:") ? null : nativeSnapshot.selected,
  };
  const mismatch = firstMismatch(expectedWire, browserWire, `${phase} estimate at ride step ${stepNumber}`);
  if (mismatch) failures.push(mismatch);
  if (phase === "selected" && isShaped) {
    const nativeDigest = nativeSnapshot.selectedShape?.spec?.geometryDigest;
    const browserDigest = browserSnapshot.selectedShape?.spec?.geometryDigest;
    if (!nativeDigest || !/^[0-9a-f]{16}$/.test(nativeDigest)) failures.push(`native shaped geometry digest at ride step ${stepNumber} is ${nativeDigest}`);
    if (browserDigest !== nativeDigest) {
      failures.push(`shaped geometry digest at ride step ${stepNumber}: native ${nativeDigest}, browser path ${browserDigest}`);
    }
  }
  const refusal = browserSnapshot.selectedPiece ? browserSnapshot.selectedPiece.refusal : browserSnapshot.selectedShape?.refusal ?? "";
  if (phase === "selected" && refusal) failures.push(`ride step ${stepNumber}: the builder refuses the selection: ${JSON.stringify(refusal)}`);
}

interface BrowserStepSnapshot {
  readonly phase: "selected" | "placed";
  readonly stepNumber: number;
  readonly isShaped: boolean;
  readonly estimates: ConstructionEstimates;
}

interface BrowserPathRun {
  /** Every builder command sent, in order, through the construction test. */
  readonly commands: readonly ConstructionAction[];
  readonly snapshots: readonly BrowserStepSnapshot[];
  readonly report: TestingReport | null;
  /** Why the browser test would stop early, as the browser driver would fail at that step. */
  readonly stopReason: string | null;
}

/** Replays the ride through the browser worker's path, recording the builder estimates the browser test reads. */
function replayBrowserPath(ride: RideFile, seed: number): BrowserPathRun {
  const replay = new WorkerReplay(seed);
  const snapshots: BrowserStepSnapshot[] = [];
  replay.dispatch({ type: "construction-select", piece: `model:${ride.model}` });
  replay.dispatch({ type: "construction-start", x: ride.station.x, y: ride.station.y, z: ride.station.z, heading: headingIndex[ride.station.heading] });
  if (replay.track().length !== 1) return { commands: replay.commands, snapshots, report: null, stopReason: "Station placement did not add exactly one track piece" };
  try {
    setCarsPerTrain(replay, ride.carsPerTrain);
  } catch (error) {
    if (error instanceof BrowserDriverRefusal) return { commands: replay.commands, snapshots, report: null, stopReason: error.message };
    throw error;
  }
  for (const [index, step] of ride.steps.entries()) {
    try {
      const stopReason = replayStep(replay, step, index + 1, snapshots);
      if (stopReason) return { commands: replay.commands, snapshots, report: null, stopReason };
    } catch (error) {
      if (error instanceof BrowserDriverRefusal) return { commands: replay.commands, snapshots, report: null, stopReason: `ride step ${index + 1}: ${error.message}` };
      throw error;
    }
  }
  if (!replay.simulation.construction_can_test()) return { commands: replay.commands, snapshots, report: null, stopReason: "Test coaster is disabled after the ride steps" };
  const report = browserTestReport(replay);
  return { commands: replay.commands, snapshots, report, stopReason: null };
}

/** Performs one ride step as place-circuit.ts does; returns why the browser driver would fail, or null. */
function replayStep(replay: WorkerReplay, step: RideStep, stepNumber: number, snapshots: BrowserStepSnapshot[]): string | null {
  if ("chain" in step) {
    if (replay.requireState().chain !== step.chain) replay.dispatch({ type: "construction-set-chain", enabled: step.chain });
    return null;
  }
  if ("close" in step) {
    if (!step.close) return null;
    if (!replay.simulation.construction_can_close()) return `ride step ${stepNumber}: Close circuit is disabled`;
    replay.dispatch({ type: "construction-close" });
    return replay.simulation.construction_can_close() ? `ride step ${stepNumber}: Close circuit stayed enabled after closing` : null;
  }
  if ("blockSections" in step) return `ride step ${stepNumber}: the browser test cannot apply saved block sections through the visible controls`;
  const isShaped = "intent" in step;
  const piece = isShaped ? selectShapedStep(replay, step) : step.place;
  if (!isShaped) selectClassicPiece(replay, piece);
  snapshots.push({ phase: "selected", stepNumber, isShaped, estimates: replay.estimates() });
  const canAppend = replay.simulation.construction_finish_preview_active() || replay.simulation.construction_can_append();
  if (!canAppend) return `ride step ${stepNumber}: cannot place ${piece}: ${replay.simulation.construction_preview_reason()}`;
  const trackCount = replay.track().length;
  replay.dispatch({ type: "construction-append" });
  const placedCount = replay.track().length - trackCount;
  if (piece.startsWith("element:") ? placedCount < 1 : placedCount !== 1) return `ride step ${stepNumber}: placing ${piece} added ${placedCount} track pieces`;
  if ("place" in step && step.launchSpeed !== undefined) setLaunchSpeed(replay, step.launchSpeed);
  snapshots.push({ phase: "placed", stepNumber, isShaped, estimates: replay.estimates() });
  return null;
}

/** Runs the construction test as the browser does: press Test, then advance ticks until the report exists. */
function browserTestReport(replay: WorkerReplay): TestingReport {
  replay.dispatch({ type: "construction-test" });
  for (let ticks = 0; ticks < browserTestTickLimit; ticks += browserTestTickChunk) {
    for (let tick = 0; tick < browserTestTickChunk; tick += 1) replay.simulation.tick();
    const report = JSON.parse(replay.simulation.construction_test_report_json()) as TestingReport | null;
    if (report) return report;
  }
  throw new Error(`The construction test produced no report within ${browserTestTickLimit} ticks`);
}

/**
 * The native design check, run alongside the WASM replay. It exits 1 when expectations fail, which still leaves its
 * estimates to compare; when it cannot run at all (for example on stale shaped geometry) this resolves to its error.
 */
function runNativeCheck(designBinary: string, ridePath: string): Promise<NativeRideCheckWithSamples | string> {
  return new Promise((resolvePromise, reject) => {
    execFile(designBinary, ["check", "--json", ridePath], { cwd: repositoryRoot, maxBuffer: 1 << 30, encoding: "utf8" }, (error, stdout, stderr) => {
      const status = error ? error.code : 0;
      if (typeof status !== "number") {
        reject(error);
        return;
      }
      if (status !== 0 && (status !== 1 || stdout === "")) {
        resolvePromise(`native design check failed: ${stderr.trim()}`);
        return;
      }
      const check = (JSON.parse(stdout) as NativeRideCheckWithSamples[])[0];
      if (!check) reject(new Error(`Native design check omitted ${ridePath}`));
      else resolvePromise(check);
    });
  });
}

/**
 * The builder commands that decide the built ride: for each run of consecutive selections only the last one, which
 * is the selection a placement or other command acts on, through the first construction test. The fast check mirrors
 * the browser driver's final selections exactly but reaches some classic pieces without the stepper clicks before
 * them, so the browser test compares this reduced list with the one the browser worker recorded.
 */
export function decisiveConstructionCommands(commands: readonly { readonly type: string }[]): { readonly type: string }[] {
  const testIndex = commands.findIndex(({ type }) => type === "construction-test");
  if (testIndex < 0) throw new Error("The builder commands never start the construction test");
  return commands
    .slice(0, testIndex + 1)
    .filter(({ type }, index, all) => type.startsWith("construction-") && !(type === "construction-select" && all.slice(index + 1).find((next) => next.type.startsWith("construction-"))?.type === "construction-select"));
}

/** The builder commands the fast check sends for a ride, as the browser driver would send them. */
export function browserPathCommands(ridePath: string): readonly ConstructionAction[] {
  ensureWasm();
  const run = replayBrowserPath(readRideFile(ridePath), browserSeed(ridePath));
  if (run.stopReason) throw new Error(`The browser path stops: ${run.stopReason}`);
  return run.commands;
}

/** The browser ride-agreement test's seed for each content ride. */
function browserSeed(ridePath: string): number {
  return ridePath.endsWith("velocity-viper.ride.json") ? 2026 : 149;
}

function compareBrowserPath(nativeCheck: NativeRideCheckWithSamples, run: BrowserPathRun): string[] {
  const failures: string[] = [];
  const expectedByStep = new Map(nativeCheck.browserEstimates.map((estimate) => [estimate.stepIndex, estimate]));
  for (const snapshot of run.snapshots) {
    const expected = expectedByStep.get(snapshot.stepNumber);
    if (!expected) {
      failures.push(`Native estimate missing for ride step ${snapshot.stepNumber}`);
      continue;
    }
    compareEstimate(failures, snapshot.phase, snapshot.stepNumber, snapshot.isShaped, expected[snapshot.phase === "selected" ? "selection" : "placement"], snapshot.estimates);
  }
  if (run.stopReason) {
    failures.push(run.stopReason);
    return failures;
  }
  if (run.snapshots.length !== nativeCheck.browserEstimates.length * 2) {
    failures.push(`compared ${run.snapshots.length} estimates; the native check recorded ${nativeCheck.browserEstimates.length * 2}`);
  }
  if (!run.report) throw new Error("A completed browser-path replay has no test report");
  failures.push(...browserAgreementResults(nativeCheck, run.report).failures);
  return failures;
}

export async function checkRideAgreement(designBinary: string, ridePath: string): Promise<RideAgreementResult> {
  ensureWasm();
  const ride = readRideFile(ridePath);
  const start = performance.now();
  const nativeCheckPromise = runNativeCheck(designBinary, ridePath);
  // Let the native process start before the synchronous WASM replay occupies this thread.
  await new Promise((resolveTimer) => setTimeout(resolveTimer, 0));
  const run = replayBrowserPath(ride, browserSeed(ridePath));
  const wasmSeconds = (performance.now() - start) / 1000;
  const nativeCheck = await nativeCheckPromise;
  const failures = typeof nativeCheck === "string" ? [nativeCheck] : compareBrowserPath(nativeCheck, run);
  return {
    ride: ride.name,
    estimateComparisons: typeof nativeCheck === "string" ? 0 : run.snapshots.length,
    failures,
    seconds: { wasm: wasmSeconds, total: (performance.now() - start) / 1000 },
  };
}
