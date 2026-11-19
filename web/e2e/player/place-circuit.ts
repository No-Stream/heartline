import { expect, type Page } from "@playwright/test";
import { metresPerSecondToDisplay } from "../../src/units";
import type { RideStep } from "./rides";

const previewNames: Record<string, string> = {
  TopHat: "Top hat", TightTwisterLeft: "Tight left turn", TightTwisterRight: "Tight right turn",
  TwisterFanLeft: "Left fan turn", TwisterFanRight: "Right fan turn",
};

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
    await expect(direct, `Piece ${piece} is not offered by the palette`).toBeVisible();
    await direct.click();
  }
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  await expect(selected).toHaveAttribute("data-selected", piece);
}

export interface PlaceCircuitOptions {
  readonly keyboard?: boolean;
  readonly carsPerTrain?: number;
  /** The step number is one-based and counts every ride-file step, including chain and close steps. */
  readonly onEstimate?: (phase: "selected" | "placed", step: RideStep, stepNumber: number) => Promise<void>;
}

function asRideStep(step: string | RideStep): RideStep {
  if (typeof step !== "string") return step;
  if (step === "+chain") return { chain: true };
  if (step === "-chain") return { chain: false };
  return { place: step };
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
  steps: readonly (string | RideStep)[],
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
    const piece = step.place;
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
    await expect(page.getByRole("button", { name: "Place piece" }), `cannot place ${piece}`).toBeEnabled();
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
    if (step.launchSpeed !== undefined) await setLaunchSpeed(page, step.launchSpeed);
    await options.onEstimate?.("placed", step, index + 1);
  }
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
}
