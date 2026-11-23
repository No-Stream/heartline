import { expect, type Page } from "@playwright/test";
import { shapeSelectionId } from "../../src/construction-shaped-elements";
import { displayToMetres, metresPerSecondToDisplay, type Units } from "../../src/units";
import { classicHillTargets, hillAttributeOrder, hillFeelOrder, hillTwistOrder, selectionFromRideIntent, shapeBaseSelectionId } from "./ride-shape-selection";
import type { RideShapeStep, RideStep } from "./rides";

export { selectionFromRideIntent };

export type PlaceCircuitShapeStep = RideShapeStep;

export type PlaceCircuitRideStep = RideStep | PlaceCircuitShapeStep;

type HillAttribute = "feel" | "height" | "span" | "twist";

const previewNames: Record<string, string> = {
  TopHat: "Top hat", TightTwisterLeft: "Tight left turn", TightTwisterRight: "Tight right turn",
  TwisterFanLeft: "Left fan turn", TwisterFanRight: "Right fan turn",
};

async function stepHill(page: Page, attribute: HillAttribute, target: string): Promise<void> {
  const value = page.locator(`[data-hill-value="${attribute}"]`);
  const editor = page.locator("[data-construction] [data-editor]");
  for (let step = 0; step < 12; step++) {
    await expect(editor).toHaveAttribute("aria-busy", "false");
    const current = (await value.textContent())?.trim() ?? "";
    if (current === target) return;
    const order = attribute === "feel" ? hillFeelOrder : hillTwistOrder;
    const currentRank = attribute === "height" || attribute === "span" ? Number.parseFloat(current) : order.indexOf(current);
    const targetRank = attribute === "height" || attribute === "span" ? Number.parseFloat(target) : order.indexOf(target);
    if (!Number.isFinite(currentRank) || currentRank < 0 || targetRank < 0) throw new Error(`Cannot select ${attribute} ${target} from ${current}`);
    const direction = currentRank < targetRank ? 1 : -1;
    const button = page.locator(`[data-hill-stepper="${attribute}-${direction}"]`);
    await expect(button, `Cannot select ${attribute} ${target} from ${current}`).toBeEnabled();
    await button.click();
    await expect(value).not.toHaveText(current);
    await expect(editor).toHaveAttribute("aria-busy", "false");
  }
  throw new Error(`Could not select hill ${attribute} ${target}`);
}

async function selectPiece(page: Page, piece: string): Promise<void> {
  const selected = page.locator("[data-construction]");
  // Read the selection only once the worker has answered every earlier command.
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  if (await selected.getAttribute("data-selected") === piece) return;
  if (piece.startsWith("element:")) {
    const element = page.locator(`[data-track-choice="${piece}"]`);
    await expect(element, `Track element ${piece} is not offered by the builder`).toBeVisible();
    await element.click();
    await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
    await expect(selected).toHaveAttribute("data-selected", piece);
    return;
  }
  const turn = /^SpeedTurnLarge(\d+)(Left|Right)$/.exec(piece);
  if (turn) {
    await page.locator('[data-piece="Straight"]').click();
    await expect(selected).toHaveAttribute("data-selected", "Straight");
    await page.getByRole("button", { name: `Turn ${turn[2].toLowerCase()}` }).click();
    await expect(selected).not.toHaveAttribute("data-selected", "Straight");
    if (await page.locator('[data-stepper-value="bank"]').textContent() !== "For speed") {
      await page.getByRole("button", { name: `Bank ${turn[2].toLowerCase()}` }).click();
    }
    await expect(page.locator('[data-stepper-value="bank"]')).toHaveText("For speed");
    await page.locator(`[data-speed-match-variant="${piece}"]`).click();
  } else {
    const direct = page.locator(`[data-piece="${piece}"]`);
    const hillTargets = classicHillTargets(piece);
    // A palette click on a hill is re-sized by the worker for the predicted speed, so hills are sized in the inspector.
    if (!hillTargets) {
      await expect(direct, `Piece ${piece} is not offered by the palette`).toBeVisible();
      await direct.click();
    } else {
      const hillSection = page.locator("[data-hill]");
      if (await hillSection.getAttribute("open") === null) await hillSection.locator("summary").click();
      await expect(hillSection.locator('[data-hill-value="feel"]')).toHaveText(/\S/);
      for (const attribute of hillAttributeOrder) await stepHill(page, attribute, hillTargets[attribute]);
    }
  }
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  await expect(selected).toHaveAttribute("data-selected", piece);
}

export interface PlaceCircuitOptions {
  readonly keyboard?: boolean;
  readonly carsPerTrain?: number;
  /** The step number is one-based and counts every ride-file step, including chain and close steps. */
  readonly onEstimate?: (phase: "selected" | "placed", step: PlaceCircuitRideStep, stepNumber: number) => Promise<void>;
}

function asRideStep(step: string | PlaceCircuitRideStep): PlaceCircuitRideStep {
  if (typeof step !== "string") return step;
  if (step === "+chain") return { chain: true };
  if (step === "-chain") return { chain: false };
  return { place: step };
}

async function readResolvedShapeDimension(page: Page, kind: "hill" | "turn"): Promise<number> {
  const readout = page.locator("[data-shape-readout]");
  const lines = await readout.locator("p").allTextContents();
  const dimensionLine = lines.find((line) => line.startsWith("Resolved size:"));
  if (!dimensionLine) throw new Error("Selected shaped element has no resolved size readout");
  const expression = kind === "hill" ? /Resolved size: ([\d,.]+) (m|ft) high/ : /Resolved size: ([\d,.]+) (m|ft) radius/;
  const match = expression.exec(dimensionLine);
  if (!match) throw new Error(`Cannot read resolved shaped size: ${dimensionLine}`);
  const displayValue = Number(match[1].replaceAll(",", ""));
  if (!Number.isFinite(displayValue)) throw new Error(`Invalid resolved shaped size: ${dimensionLine}`);
  const units: Units = match[2] === "m" ? "metric" : "imperial";
  return displayToMetres(displayValue, units);
}

function parseShapeForce(line: string): number {
  const match = /Airtime target: ([−-]?[\d,.]+) g/.exec(line);
  if (!match) throw new Error(`Cannot read shaped airtime target: ${line}`);
  const value = Number(match[1].replace("−", "-").replaceAll(",", ""));
  if (!Number.isFinite(value)) throw new Error(`Invalid shaped airtime target: ${line}`);
  return value;
}

async function stepShapeValue(page: Page, attribute: "strength" | "size" | "radius" | "peakG" | "topFeel", direction: -1 | 1): Promise<void> {
  const value = page.locator(`[data-shape-value="${attribute}"]`);
  const editor = page.locator("[data-construction] [data-editor]");
  const current = (await value.textContent())?.trim();
  if (!current) throw new Error(`Missing shaped element ${attribute} value`);
  const button = page.locator(`[data-shape-stepper="${attribute}-${direction}"]`);
  await expect(button, `Cannot adjust shaped element ${attribute}`).toBeEnabled();
  await button.click();
  // The editor stays aria-busy="false" until the worker answers the click, so wait for the
  // worker-confirmed value to change before reading any readout.
  await expect(value).not.toHaveText(current);
  await expect(editor).toHaveAttribute("aria-busy", "false");
}

async function selectShapedStep(page: Page, step: PlaceCircuitShapeStep): Promise<string> {
  const intentSelection = selectionFromRideIntent(step.intent);
  const baseId = shapeBaseSelectionId(intentSelection);
  if (intentSelection.kind === "hill") {
    const selected = page.locator("[data-construction]");
    const editor = page.locator("[data-construction] [data-editor]");
    const paletteChoice = page.locator(`[data-piece="${baseId}"]`);
    await expect(editor).toHaveAttribute("aria-busy", "false");
    await expect(paletteChoice, `Shaped hill ${baseId} is not offered by the palette`).toBeVisible();
    await paletteChoice.click();
    await expect(editor).toHaveAttribute("aria-busy", "false");

    const targetLine = page.locator("[data-shape-readout] p").filter({ hasText: "Airtime target:" });
    const defaultStrength = parseShapeForce((await targetLine.textContent()) ?? "");
    const defaultId = shapeSelectionId({
      ...intentSelection,
      strengthG: defaultStrength,
      size: { kind: "auto" },
    });
    await expect(selected).toHaveAttribute("data-selected", defaultId);
  } else {
    await selectPiece(page, baseId);
  }
  await expect(page.locator("[data-shape-readout]")).toBeVisible();

  if (intentSelection.kind === "hill") {
    const targetStrength = "hill" in step.intent && step.spec.kind === "hill"
      ? step.intent.hill.strength_g
      : undefined;
    if (targetStrength !== undefined) {
      const readout = page.locator("[data-shape-readout] p").filter({ hasText: "Airtime target:" });
      let currentStrength = parseShapeForce((await readout.textContent()) ?? "");
      const strengthSteps = (currentStrength - targetStrength) / 0.1;
      if (Math.abs(strengthSteps - Math.round(strengthSteps)) > 1e-5) {
        throw new Error(`Visible strength stepper cannot select ${targetStrength} g from ${currentStrength} g in 0.1 g steps`);
      }
      while (Math.abs(currentStrength - targetStrength) > 1e-5) {
        const direction = currentStrength > targetStrength ? 1 : -1;
        const expectedStrength = currentStrength - direction * 0.1;
        await stepShapeValue(page, "strength", direction);
        currentStrength = parseShapeForce((await readout.textContent()) ?? "");
        expect(currentStrength, `Strength stepper must move to ${expectedStrength} g while selecting ${targetStrength} g`)
          .toBeCloseTo(expectedStrength, 5);
      }
    }
    if (intentSelection.size.kind === "height") {
      await setFixedShapeDimension(page, "hill", intentSelection.size.heightMetres);
    }
  } else if (intentSelection.kind === "turn") {
    if (intentSelection.size.kind === "radius") {
      await setFixedShapeDimension(page, "turn", intentSelection.size.radiusMetres);
    }
  } else {
    const readPeakG = async (): Promise<number> => {
      const text = (await page.locator('[data-shape-value="peakG"]').textContent()) ?? "";
      const match = /([0-9]+(?:\.[0-9]+)?)/.exec(text);
      if (!match) throw new Error(`Invalid shaped inversion peak g readout: ${text}`);
      return Number(match[1]);
    };
    let currentPeakG = await readPeakG();
    while (Math.abs(currentPeakG - intentSelection.peakG) > 1e-5) {
      await stepShapeValue(page, "peakG", currentPeakG < intentSelection.peakG ? 1 : -1);
      currentPeakG = await readPeakG();
    }
    if (intentSelection.kind !== "zeroGRoll") {
      const topFeelOrder = ["Floaty", "Neutral", "Pressed in"] as const;
      const targetIndex = intentSelection.topFeel === "floaty" ? 0 : intentSelection.topFeel === "neutral" ? 1 : 2;
      const readTopFeelIndex = async (): Promise<number> => {
        const label = ((await page.locator('[data-shape-value="topFeel"]').textContent()) ?? "").trim();
        const index = topFeelOrder.indexOf(label as (typeof topFeelOrder)[number]);
        if (index < 0) throw new Error(`Invalid shaped inversion top feel readout: ${label}`);
        return index;
      };
      let currentIndex = await readTopFeelIndex();
      while (currentIndex !== targetIndex) {
        await stepShapeValue(page, "topFeel", currentIndex < targetIndex ? 1 : -1);
        currentIndex = await readTopFeelIndex();
      }
    }
  }

  const expectedId = shapeSelectionId(intentSelection);
  const selectedId = await page.locator("[data-construction]").getAttribute("data-selected");
  if (selectedId !== expectedId) {
    throw new Error(`Visible shaped-element controls selected ${selectedId}, but ride intent requires ${expectedId}`);
  }
  return expectedId;
}

async function setFixedShapeDimension(page: Page, kind: "hill" | "turn", targetMetres: number): Promise<void> {
  const attribute = kind === "hill" ? "size" : "radius";
  if (Math.abs(targetMetres / 2 - Math.round(targetMetres / 2)) > 1e-8) {
    throw new Error(`Visible shaped element ${attribute} stepper only selects 2 m increments; ride intent requests ${targetMetres} m`);
  }
  const targetGridValue = Math.round(targetMetres / 2) * 2;
  let resolvedSize = await readResolvedShapeDimension(page, kind);
  let currentGridValue = Math.round(resolvedSize / 2) * 2;
  let selectedFixedSize = false;
  while (currentGridValue !== targetGridValue || !selectedFixedSize) {
    const direction: -1 | 1 = currentGridValue < targetGridValue ? 1 : -1;
    await stepShapeValue(page, attribute, direction);
    const valueLabel = (await page.locator(`[data-shape-value="${attribute}"]`).textContent())?.trim() ?? "";
    if (valueLabel === "Auto") {
      throw new Error(`The visible ${attribute} stepper did not select a fixed shaped element size`);
    }
    selectedFixedSize = true;
    resolvedSize = await readResolvedShapeDimension(page, kind);
    currentGridValue = Math.round(resolvedSize / 2) * 2;
  }
}

async function setCarsPerTrain(page: Page, carsPerTrain: number): Promise<void> {
  if (!Number.isInteger(carsPerTrain) || carsPerTrain < 1) throw new RangeError(`Invalid cars per train: ${carsPerTrain}`);
  await page.locator("[data-construction] [data-editor]").waitFor({ state: "attached" });
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  const operateTab = page.getByRole("tablist", { name: "Coaster builder sections" }).getByRole("tab", { name: "Operate", exact: true });
  if (await operateTab.getAttribute("aria-selected") !== "true") await operateTab.click();
  await expect(operateTab).toHaveAttribute("aria-selected", "true");
  const value = page.locator("[data-cars-per-train]");
  const fewer = page.getByRole("button", { name: "Remove one car per train" });
  const more = page.getByRole("button", { name: "Add one car per train" });
  while (Number((await value.textContent())?.trim()) !== carsPerTrain) {
    const current = Number((await value.textContent())?.trim());
    const button = carsPerTrain < current ? fewer : more;
    await expect(button, `Cannot set cars per train from ${current} to ${carsPerTrain}`).toBeEnabled();
    await button.click();
    await expect(value).toHaveText(String(carsPerTrain < current ? current - 1 : current + 1));
    await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  }
  const buildTab = page.getByRole("tablist", { name: "Coaster builder sections" }).getByRole("tab", { name: "Build", exact: true });
  await buildTab.click();
  await expect(buildTab).toHaveAttribute("aria-selected", "true");
}

async function setLaunchSpeed(page: Page, launchSpeed: number): Promise<void> {
  if (!Number.isFinite(launchSpeed) || launchSpeed <= 0) throw new RangeError(`Invalid launch speed: ${launchSpeed}`);
  const unitsButton = page.getByRole("button", { name: "Units", exact: true });
  const units = await unitsButton.getAttribute("data-units");
  if (units !== "imperial" && units !== "metric") throw new Error(`Unknown speed units: ${units}`);
  const launchTarget = page.locator("[data-launch-list] li input").last();
  await expect(launchTarget, "Placed launch piece has no target speed control").toBeVisible();
  const exactMinimumAttribute = await launchTarget.getAttribute("data-minimum-display-speed");
  const exactMaximumAttribute = await launchTarget.getAttribute("data-maximum-display-speed");
  if (exactMinimumAttribute === null || exactMaximumAttribute === null) throw new Error("Launch target is missing its exact speed bounds");
  const exactMinimum = Number(exactMinimumAttribute);
  const exactMaximum = Number(exactMaximumAttribute);
  if (!Number.isFinite(exactMinimum) || !Number.isFinite(exactMaximum)) throw new Error("Launch target has invalid exact speed bounds");
  const displaySpeed = metresPerSecondToDisplay(launchSpeed, units);
  if (displaySpeed < exactMinimum || displaySpeed > exactMaximum) {
    throw new RangeError(`Ride launch speed ${launchSpeed} m/s is outside the builder bounds ${exactMinimum}–${exactMaximum} ${units}`);
  }
  await launchTarget.fill(String(displaySpeed));
  await launchTarget.press("Tab");
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  await expect(launchTarget).toHaveAttribute("data-launch-target-speed", String(launchSpeed));
}

/** Place exact ride steps with visible builder controls, waiting for every worker-confirmed change. */
export async function placeCircuit(
  page: Page,
  steps: readonly (string | PlaceCircuitRideStep)[],
  keyboardOrOptions: boolean | PlaceCircuitOptions = false,
  onEstimate?: PlaceCircuitOptions["onEstimate"],
): Promise<void> {
  const options = typeof keyboardOrOptions === "boolean" ? { keyboard: keyboardOrOptions, onEstimate } : keyboardOrOptions;
  if (options.carsPerTrain !== undefined) await setCarsPerTrain(page, options.carsPerTrain);
  let trackCount = await page.locator("[data-track] li").count();
  let chainEnabled = false;
  for (const [index, rawStep] of steps.entries()) {
    const step = asRideStep(rawStep);
    if ("chain" in step) {
      chainEnabled = step.chain;
      const construction = page.locator("[data-construction]");
      await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
      if (await construction.getAttribute("data-chain") !== String(step.chain)) {
        await page.getByRole("button", { name: "Chain lift" }).click();
      }
      await expect(construction).toHaveAttribute("data-chain", String(step.chain));
      await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
      continue;
    }
    if ("close" in step) {
      if (!step.close) continue;
      const close = page.getByRole("button", { name: "Close circuit" });
      await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
      await expect(close).toBeEnabled();
      await close.click();
      await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
      await expect(close).toBeDisabled();
      continue;
    }
    if ("blockSections" in step) {
      throw new Error("placeCircuit cannot apply saved block sections through the visible controls");
    }
    const piece = "intent" in step ? await selectShapedStep(page, step) : step.place;
    if ((options.keyboard ?? false) && (piece === "LaunchLong" || piece === "LaunchShort")) {
      await page.locator("canvas").focus();
      const selected = page.locator('[data-piece][aria-checked="true"]');
      for (let step = 0; await selected.getAttribute("data-piece") !== piece; step++) {
        if (step >= 100) throw new Error(`Piece ${piece} is not offered by the keyboard palette`);
        const previous = await selected.getAttribute("data-piece");
        await page.keyboard.press("]");
        await expect(selected).not.toHaveAttribute("data-piece", previous!);
      }
    } else {
      await selectPiece(page, piece);
    }
    await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
    await options.onEstimate?.("selected", step, index + 1);
    if (chainEnabled && piece === "TurnSmallStandalone") {
      await expect(page.locator("[data-next] [data-preview-name]")).toHaveText("Climbing small left turn");
    }
    if (piece in previewNames) {
      await expect(page.locator("[data-next] [data-preview-name]")).toHaveText(previewNames[piece]);
    }
    const placeButton = page.getByRole("button", { name: "Place piece" });
    if (await placeButton.isDisabled()) {
      const shapeReadout = await page.locator("[data-shape-readout]").innerText();
      const constructionMessage = await page.locator("[data-construction] [data-message]").innerText();
      throw new Error(
        `cannot place ${piece}; worker shape readout: ${JSON.stringify(shapeReadout)}; `
        + `construction message: ${JSON.stringify(constructionMessage)}`,
      );
    }
    await expect(placeButton, `cannot place ${piece}`).toBeEnabled();
    if (options.keyboard ?? false) {
      await page.locator("canvas").focus();
      await page.keyboard.press("b");
    } else {
      await page.getByRole("button", { name: "Place piece" }).click();
    }
    const track = page.locator("[data-track] li");
    if (piece.startsWith("element:")) {
      await expect.poll(async () => track.count(), { message: `place track element ${piece}` }).toBeGreaterThan(trackCount);
      trackCount = await track.count();
    } else {
      await expect(track).toHaveCount(++trackCount);
    }
    await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
    if ("place" in step && step.launchSpeed !== undefined) await setLaunchSpeed(page, step.launchSpeed);
    await options.onEstimate?.("placed", step, index + 1);
  }
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
}
