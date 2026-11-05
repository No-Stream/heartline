import { expect, test } from "@playwright/test";

test("sound preference works in both interface themes and survives reload", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);

  const soundButton = page.getByRole("button", { name: "Sound" });
  const volume = page.getByRole("slider", { name: "Volume" });
  await expect(soundButton).toHaveAttribute("aria-pressed", "true");
  await expect(soundButton).toHaveAttribute("data-sound", "on");
  await expect(volume).toHaveValue("70");

  for (const theme of ["light", "dark"] as const) {
    await page.locator("html").evaluate((html, nextTheme) => { html.dataset.theme = nextTheme; }, theme);
    await expect(soundButton).toBeVisible();
    const bounds = await soundButton.boundingBox();
    expect(bounds?.width).toBeGreaterThan(0);
    expect(bounds?.height).toBeGreaterThan(0);
  }

  await page.locator("canvas").focus();
  await page.keyboard.press("m");
  await expect(soundButton).toHaveAttribute("aria-pressed", "false");
  await expect(soundButton).toHaveAttribute("data-sound", "off");
  await page.keyboard.press("m");
  await expect(soundButton).toHaveAttribute("data-sound", "on");

  await volume.fill("42");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("theme-park-sim.sound"))).toBe('{"volume":0.42,"muted":false}');
  await page.reload();
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await expect(page.getByRole("button", { name: "Sound" })).toHaveAttribute("data-sound", "on");
  await expect(page.getByRole("slider", { name: "Volume" })).toHaveValue("42");
});
