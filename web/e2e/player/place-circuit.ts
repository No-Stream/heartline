import { expect, type Page } from "@playwright/test";

type HillAttribute = "feel" | "height" | "span" | "twist";

const hillFeelOrder = ["Ejector", "Strong floater", "Floater", "Speed hill", "Zero-g roll", "Stall"];
const hillTwistOrder = ["Outer bank left", "Upright", "Outer bank right", "Roll left", "Roll right"];
const previewNames: Record<string, string> = {
  TopHat: "Top hat", TightTwisterLeft: "Tight left turn", TightTwisterRight: "Tight right turn",
  TwisterFanLeft: "Left fan turn", TwisterFanRight: "Right fan turn",
};

async function stepHill(page: Page, attribute: HillAttribute, target: string): Promise<void> {
  const value = page.locator(`[data-hill-value="${attribute}"]`);
  for (let step = 0; step < 12; step++) {
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
  }
  throw new Error(`Could not select hill ${attribute} ${target}`);
}

async function selectPiece(page: Page, piece: string): Promise<void> {
  const selected = page.locator("[data-construction]");
  // Read the selection only once the worker has answered every earlier command.
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  if (await selected.getAttribute("data-selected") === piece) return;
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
    const hill = /^(WoodenEjector|Ejector|Floater|Family|Speed)Hill(\d+)x(\d+)$/.exec(piece);
    const roll = /^ZeroGRoll(Fast|Compact)(Left|Right)$/.exec(piece);
    // A palette click on a hill is re-sized by the worker for the predicted speed, so hills are sized in the inspector.
    if (!hill && !roll) {
      await expect(direct, `Piece ${piece} is not offered by the palette`).toBeVisible();
      await direct.click();
    } else {
      const hillSection = page.locator("[data-hill]");
      if (await hillSection.getAttribute("open") === null) await hillSection.locator("summary").click();
      await expect(hillSection.locator('[data-hill-value="feel"]')).toHaveText(/\S/);
      const feel = roll ? "Zero-g roll" : piece === "WoodenEjectorHill8x64" ? "Strong floater"
        : hill![1].includes("Ejector") ? "Ejector" : hill![1] === "Floater" || hill![1] === "Family" ? "Floater" : "Speed hill";
      await stepHill(page, "feel", feel);
      await stepHill(page, "twist", roll ? `Roll ${roll[2].toLowerCase()}` : "Upright");
      await stepHill(page, "height", `${roll ? (roll[1] === "Fast" ? 8 : 10) : hill![2]} m`);
      await stepHill(page, "span", `${roll ? (roll[1] === "Fast" ? 86 : 80) : hill![3]} m`);
    }
  }
  await expect(page.locator("[data-construction] [data-editor]")).toHaveAttribute("aria-busy", "false");
  await expect(selected).toHaveAttribute("data-selected", piece);
}

/** Place the exact fixture with visible controls, waiting for every worker-confirmed piece. */
export async function placeCircuit(page: Page, pieces: readonly string[], keyboard = false): Promise<void> {
  let count = 1;
  let chainEnabled = false;
  for (const piece of pieces) {
    if (piece === "+chain" || piece === "-chain") {
      chainEnabled = piece === "+chain";
      await page.getByRole("button", { name: "Chain lift" }).click();
      await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(chainEnabled));
      continue;
    }
    if (keyboard && (piece === "LaunchLong" || piece === "LaunchShort")) {
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
    if (chainEnabled && piece === "TurnSmallStandalone") {
      await expect(page.locator("[data-next] [data-preview-name]")).toHaveText("Climbing small left turn");
    }
    if (piece in previewNames) {
      await expect(page.locator("[data-next] [data-preview-name]")).toHaveText(previewNames[piece]);
    }
    await expect(page.getByRole("button", { name: "Place piece" }), `cannot place ${piece}`).toBeEnabled();
    if (keyboard) {
      await page.locator("canvas").focus();
      await page.keyboard.press("b");
    } else {
      await page.getByRole("button", { name: "Place piece" }).click();
    }
    await expect(page.locator("[data-track] li")).toHaveCount(++count);
  }
}
