import { expect, test } from "@playwright/test";
import type { SoundFrame } from "../src/audio/mapping";

const frame = (screams: SoundFrame["screams"] = []): SoundFrame => ({ trains: [], screams, crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: [], eventsMuted: false });

test("rider screams produce formant energy and stop when events are muted @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const scream = frame([{ train: 0, pan: 0, audibility: 1, intensity: 1 }]);
  const results = await page.evaluate(async ({ scream, quiet }) => {
    const render = (frames: { time: number; frame: SoundFrame }[]) => window.gameTest.audio.renderOffline({ duration: 2, volume: 1, includeSamples: true, frames });
    return Promise.all([render([{ time: 0, frame: scream }]), render([{ time: 0, frame: quiet }]), render([{ time: 0, frame: scream }, { time: 0.2, frame: { ...quiet, eventsMuted: true } }])]);
  }, { scream, quiet: frame() });
  const [active, silent, muted] = results.map((result) => result.channels[0]!);
  expect(active!.bands.from200To2000).toBeGreaterThan(silent!.bands.from200To2000 + 0.005);
  expect(active!.peak).toBeLessThan(1);
  const samples = muted!.samples!;
  const late = samples.slice(24000, 43200);
  expect(Math.sqrt(late.reduce((sum, sample) => sum + sample * sample, 0) / late.length)).toBeLessThan(0.0001);
});

test("simultaneous drops occupy separate groups and a suppressed repeat adds none @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const drops = frame([
    { train: 0, pan: -0.4, audibility: 1, intensity: 0.8 },
    { train: 2, pan: 0.4, audibility: 1, intensity: 0.8 },
  ]);
  const result = await page.evaluate(({ drops, quiet }) => window.gameTest.audio.renderOffline({ duration: 2, includeVoiceTimeline: true, frames: [
    { time: 0, frame: quiet },
    { time: 0.1, frame: drops },
    { time: 0.3, frame: quiet },
    { time: 1.9, frame: quiet },
  ] }), { drops, quiet: frame() });
  expect(result.voiceTimeline!.map((voices) => voices.screams)).toEqual([0, 2, 2, 0]);
});

test("the occupied demo train produces a live scream @slow", async ({ page }) => {
  test.setTimeout(150_000);
  await page.goto("/?park=demo&seed=42");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.locator("canvas").click();
  const speed = page.getByRole("button", { name: "Game speed" });
  await speed.click();
  await speed.click();
  await expect(speed).toHaveText("×4");
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.activeVoices().screams), { timeout: 110_000, intervals: [50] }).toBeGreaterThanOrEqual(1);
});
