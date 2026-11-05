import { expect, test, type Page } from "@playwright/test";
import type { OfflineScript } from "../src/audio/engine";
import type { SoundFrame, UiEvent } from "../src/audio/mapping";
import { placePiece, placeStation, startPark } from "./player/driver";

const quietFrame = (ui: UiEvent[] = []): SoundFrame => ({ trains: [], screams: [], crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui, eventsMuted: false });

async function renderOffline(page: Page, script: OfflineScript) {
  return page.evaluate((offlineScript) => window.gameTest.audio.renderOffline(offlineScript), script);
}

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

test("each build and interface sound peaks within 50 ms of its event, the error stays low, and a fast run stays below clipping @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const events: UiEvent[] = [{ sound: "place", material: "steel" }, { sound: "place", material: "wood" }, { sound: "place", material: "hybrid" }, { sound: "build" }, { sound: "undo" }, { sound: "error" }, { sound: "finish" }];
  for (const event of events) {
    const result = await renderOffline(page, { duration: 1.5, frames: [{ time: 0, frame: quietFrame() }, { time: 0.2, frame: quietFrame([event]) }] });
    for (const channel of result.channels) {
      expect(channel.peak, event.sound).toBeGreaterThan(0.05);
      expect(channel.peak, event.sound).toBeLessThan(1);
      expect(channel.peakTime - 0.2, event.sound).toBeGreaterThanOrEqual(0);
      expect(channel.peakTime - 0.2, event.sound).toBeLessThan(0.05);
      if (event.sound === "error") expect(channel.bands.above1000, "error energy above 1 kHz").toBeLessThan(0.35 * channel.rms);
    }
  }
  const run = await renderOffline(page, { duration: 3, frames: Array.from({ length: 20 }, (_, index) => ({ time: index * 0.12, frame: quietFrame([{ sound: "place", material: "steel" }]) })) });
  for (const channel of run.channels) expect(channel.peak).toBeLessThan(1);
  const muted = await renderOffline(page, { duration: 1, muted: true, frames: [{ time: 0.1, frame: quietFrame([{ sound: "finish" }]) }] });
  for (const channel of muted.channels) expect(channel.peak).toBeLessThan(1e-4);
});

test("placing, undoing and refusing a Timber Tumbler piece request the matching interface sounds @fast", async ({ page }) => {
  test.setTimeout(120_000);
  const lastUiEvent = () => page.evaluate(() => window.gameTest.audio.lastUiEvent());
  await startPark(page, { park: "new", seed: 2026, speed: 1 });
  await placeStation(page, { model: "hybrid", height: 12, tile: { x: 241, y: 249, z: 2 }, direction: { x: 241, y: 251, z: 2 } });
  await page.getByRole("button", { name: "Chain lift", exact: true }).click();
  await page.getByRole("radio", { name: "Begin gentle descent", exact: true }).click();
  await expect(page.locator("[data-construction] [data-message]")).toHaveText("No chain lift on this piece");
  await expect.poll(lastUiEvent).toBe("error");
  await page.getByRole("radio", { name: "Begin gentle climb", exact: true }).click();
  await placePiece(page);
  await expect(page.locator("[data-track] li")).toHaveCount(2);
  await expect.poll(lastUiEvent).toBe("place");
  await page.keyboard.press("Backspace");
  await expect(page.locator("[data-track] li")).toHaveCount(1);
  await expect.poll(lastUiEvent).toBe("undo");
});
