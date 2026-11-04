import { expect, test, type Page } from "@playwright/test";
import { placePiece, placeStation, startPark } from "./player/driver";

declare global { interface Window { workerChain?: boolean } }

/** Records the chain flag from each construction state the worker posts, so the test can compare the panel with it. */
async function recordWorkerChain(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options);
        this.addEventListener("message", (event: MessageEvent<{ type?: string; chain?: boolean }>) => {
          if (event.data.type === "construction-state") window.workerChain = event.data.chain;
        });
      }
    };
  });
}

async function expectChain(page: Page, enabled: boolean): Promise<void> {
  await expect.poll(() => page.evaluate(() => window.workerChain)).toBe(enabled);
  await expect(page.getByRole("button", { name: "Chain lift", exact: true })).toHaveAttribute("aria-pressed", String(enabled));
  await expect(page.locator("[data-construction]")).toHaveAttribute("data-chain", String(enabled));
}

test("the chain lift toggle follows the worker, refuses pieces without a chain, and builds a curved lift", async ({ page }) => {
  test.setTimeout(120_000);
  await recordWorkerChain(page);
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  const chain = page.getByRole("button", { name: "Chain lift", exact: true });
  await expect(chain).toHaveAttribute("aria-keyshortcuts", "H");
  await expectChain(page, false);

  await chain.click();
  await expectChain(page, true);
  await page.keyboard.press("h");
  await expectChain(page, false);
  await page.keyboard.press("h");
  await expectChain(page, true);

  const descent = page.getByRole("radio", { name: "Begin gentle descent", exact: true });
  await expect(descent).toHaveClass(/piece-tile--refused/);
  await descent.hover();
  await expect(descent.getByRole("tooltip")).toBeVisible();
  await expect(descent.getByRole("tooltip")).toContainText("No chain lift on this piece");
  await descent.click();
  await expect(page.locator("[data-construction] [data-message]")).toHaveText("No chain lift on this piece");
  await expect(page.getByRole("button", { name: "Place piece" })).toBeDisabled();

  await page.getByRole("radio", { name: "Begin gentle climb", exact: true }).click();
  await placePiece(page);
  await page.getByRole("radio", { name: "Gentle climb", exact: true }).click();
  await placePiece(page);
  await page.getByRole("button", { name: "Turn left", exact: true }).click();
  await expect(page.locator("[data-next] [data-preview-name]")).toHaveText("Climbing small left turn");
  await expect(page.getByRole("button", { name: "Place piece" })).toBeEnabled();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(4);
  await expect(page.locator("[data-track] li").last()).toHaveAttribute("title", "Climbing small left turn");
  await expectChain(page, true);

  await chain.click();
  await expectChain(page, false);
  await expect(page.getByRole("radio", { name: "End gentle climb", exact: true })).not.toHaveClass(/piece-tile--refused/);
});
