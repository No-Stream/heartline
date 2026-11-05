import { stat } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { openTool } from "./game-page";

test("park actions remain accessible as the game bar fits different widths", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const park = page.getByRole("button", { name: "Park", exact: true });
  let previousFitLevel = await page.locator(".game-bar").getAttribute("data-fit-level");
  for (const width of [800, 520, 1280]) {
    await page.setViewportSize({ width, height: 720 });
    await expect.poll(() => page.locator(".game-bar").getAttribute("data-fit-level")).not.toBe(previousFitLevel);
    previousFitLevel = await page.locator(".game-bar").getAttribute("data-fit-level");
    const overflows = await page.locator(".game-bar").evaluate((bar) => {
      const styles = getComputedStyle(bar);
      const children = [...bar.children] as HTMLElement[];
      const contentWidth = children.reduce((sum, child) => sum + child.scrollWidth, 0)
        + (children.length - 1) * Number.parseFloat(styles.columnGap)
        + Number.parseFloat(styles.paddingLeft) + Number.parseFloat(styles.paddingRight);
      return contentWidth > bar.clientWidth;
    });
    expect(overflows).toBe(false);
    await expect(park).toHaveAttribute("aria-expanded", "false");
    await park.click();
    await expect(park).toHaveAttribute("aria-expanded", "true");
    for (const name of ["Save park", "Load park", "Export park"]) {
      await expect(page.getByRole("button", { name })).toBeInViewport();
    }
    await expect(page.getByLabel("Import park file")).toBeInViewport();
    await park.click();
  }
});

test("saves, loads, exports and imports a park through the interface", async ({ page }) => {
  await page.goto("/?test=1&park=demo&seed=2026");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Facilities");
  const entranceId = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "entrance")?.id);
  if (entranceId === undefined) throw new Error("The demo park entrance is missing");
  const price = () => page.evaluate((id) => window.gameTest.facilities().find((facility) => facility.id === id)?.price, entranceId);
  const originalPrice = await price();
  await page.getByRole("button", { name: `Raise the price of entrance ${entranceId}` }).click();
  await expect.poll(price).not.toBe(originalPrice);
  const savedPrice = await price();
  await page.getByRole("button", { name: "Park", exact: true }).click();
  await page.getByRole("button", { name: "Save park" }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toContainText("Park saved");
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export park" }).click();
  const download = await downloadPromise;
  const downloadPath = await download.path();
  if (!downloadPath) throw new Error("Park export has no file path");
  await test.info().attach("demo-save-size", { body: String((await stat(downloadPath)).size), contentType: "text/plain" });
  await page.reload();
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await expect.poll(price).toBe(originalPrice);
  await page.getByRole("button", { name: "Park", exact: true }).click();
  await page.getByRole("button", { name: "Load park" }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toContainText("Park loaded");
  await expect.poll(price).toBe(savedPrice);
  await page.getByRole("button", { name: "Park", exact: true }).click();
  await openTool(page, "Facilities");
  await page.getByRole("button", { name: `Raise the price of entrance ${entranceId}` }).click();
  await expect.poll(price).not.toBe(savedPrice);
  await page.getByRole("button", { name: "Park", exact: true }).click();
  await page.getByLabel("Import park file").setInputFiles(downloadPath);
  await expect(page.locator(".park-saves").getByRole("status")).toContainText("Park imported");
  await expect.poll(price).toBe(savedPrice);
  await page.getByLabel("Import park file").setInputFiles({ name: "broken.park", mimeType: "application/octet-stream", buffer: Buffer.from("broken") });
  await expect(page.locator(".park-saves").getByRole("status")).toContainText(/could not|invalid|unsupported/i);
  await expect.poll(price).toBe(savedPrice);
  await page.getByRole("button", { name: "Park", exact: true }).click();
  await openTool(page, "Watch");
  await page.getByRole("button", { name: "Export session" }).click();
  await expect(page.locator(".park-saves").getByRole("status")).toContainText("Session export is unavailable after loading a park");
});
