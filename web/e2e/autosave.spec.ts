import { expect, test, type Page } from "@playwright/test";
import { openTool } from "./game-page";

test("autosaves rotate through the configured slots and restore a selected park state", async ({ page }) => {
  await page.clock.install({ time: new Date("2026-11-05T19:44:00Z") });
  await page.goto("/?park=demo&seed=2026");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.clock.pauseAt(new Date("2026-11-05T19:45:00Z"));

  const parkToggle = page.getByRole("button", { name: "Park", exact: true });
  const autosaveRows = page.locator("[data-autosaves] [data-autosave-id]");
  await parkToggle.click();
  await page.getByLabel("Autosave interval (minutes)").fill("1");
  await page.getByLabel("Autosave slots").fill("2");
  await page.getByRole("button", { name: "Save autosave settings" }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toHaveText("Autosave settings saved.");
  await expect(page.locator("[data-autosaves] [data-autosave-id]")).toHaveCount(0);
  await parkToggle.click();

  await openTool(page, "Facilities");
  const entranceId = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "entrance")?.id);
  if (entranceId === undefined) throw new Error("The demo park entrance is missing");
  const readEntrancePrice = () => page.evaluate((id) => window.gameTest.facilities().find(({ id: currentId }) => currentId === id)?.price, entranceId);
  const raiseEntrancePrice = async (): Promise<number> => {
    if (await parkToggle.getAttribute("aria-expanded") === "true") await parkToggle.click();
    const priceBefore = await readEntrancePrice();
    if (priceBefore === undefined) throw new Error("The demo park entrance price is missing");
    await page.getByRole("button", { name: `Raise the price of entrance ${entranceId}` }).click();
    await expect.poll(readEntrancePrice).toBe(priceBefore + 1);
    const priceAfter = await readEntrancePrice();
    if (priceAfter === undefined) throw new Error("The demo park entrance price is missing");
    return priceAfter;
  };
  const showAutosaves = async (): Promise<void> => {
    if (await parkToggle.getAttribute("aria-expanded") === "false") await parkToggle.click();
  };
  const readAutosaveIds = () => autosaveRows.evaluateAll((entries) => entries.map((entry) => (entry as HTMLElement).dataset.autosaveId!));
  const waitForAutosaveCount = async (count: number): Promise<void> => {
    await showAutosaves();
    await expect(autosaveRows).toHaveCount(count, { timeout: 30_000 });
  };

  const firstSavedPrice = await raiseEntrancePrice();
  await page.clock.fastForward("01:00");
  await waitForAutosaveCount(1);
  const firstAutosaveId = (await readAutosaveIds())[0]!;
  const firstAutosave = page.locator(`[data-autosave-id="${firstAutosaveId}"]`);
  await expect(firstAutosave).toContainText("Saved");
  await expect(firstAutosave).toContainText("Park age");

  const secondSavedPrice = await raiseEntrancePrice();
  await page.clock.fastForward("01:00");
  await waitForAutosaveCount(2);
  const secondAutosaveIds = await readAutosaveIds();
  const secondAutosaveId = secondAutosaveIds.find((id) => id !== firstAutosaveId);
  if (!secondAutosaveId) throw new Error("The second autosave was not added");

  await raiseEntrancePrice();
  await page.clock.fastForward("01:00");
  await waitForAutosaveCount(2);
  const rotatedAutosaveIds = await readAutosaveIds();
  expect(rotatedAutosaveIds).toContain(secondAutosaveId);
  expect(rotatedAutosaveIds).not.toContain(firstAutosaveId);

  await raiseEntrancePrice();
  if (await parkToggle.getAttribute("aria-expanded") === "false") await parkToggle.click();
  await page.locator(`[data-autosave-id="${secondAutosaveId}"]`).getByRole("button", { name: "Load autosave", exact: true }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toHaveText("Autosave loaded.");
  await expect.poll(readEntrancePrice).toBe(secondSavedPrice);
  expect(secondSavedPrice).toBe(firstSavedPrice + 1);

  await page.locator(`[data-autosave-id="${secondAutosaveId}"]`).getByRole("button", { name: "Delete autosave", exact: true }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toHaveText("Autosave deleted.");
  await expect(autosaveRows).toHaveCount(1, { timeout: 30_000 });
});
