import { expect, test } from "@playwright/test";

test("audio unlocks on a gesture and mute suspends it @fast", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  expect(await page.evaluate(() => window.gameTest.audio.state())).toBe("locked");
  await page.locator("canvas").click();
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.state())).toBe("running");
  await page.keyboard.press("m");
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.state())).toBe("suspended");
  await expect(page.getByRole("button", { name: "Sound", exact: true })).toHaveAttribute("data-sound", "off");
  expect(errors).toEqual([]);
});

test("frozen mode stays locked and offline empty frames are silent @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.locator("canvas").click();
  expect(await page.evaluate(() => window.gameTest.audio.state())).toBe("locked");
  const result = await page.evaluate(() => window.gameTest.audio.renderOffline({
    duration: 1,
    frames: [{ time: 0, frame: { trains: [], screams: [], crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: [], eventsMuted: false } }],
  }));
  expect(result.channels).toHaveLength(2);
  for (const channel of result.channels) {
    expect(channel.peak).toBeLessThan(1e-4);
    expect(channel.rms).toBeLessThan(1e-4);
  }
  await page.evaluate(() => window.gameTest.audio.unlock());
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.state())).toBe("running");
});
