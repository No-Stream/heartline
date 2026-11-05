import { expect, test } from "@playwright/test";
import type { SoundFrame } from "../src/audio/mapping";

const frame = (crowd: SoundFrame["crowd"], ambience: SoundFrame["ambience"]): SoundFrame => ({
  trains: [], screams: [], crowd, ambience, ui: [], eventsMuted: false,
});

test("crowd and day and night ambience produce distinct sounds @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const results = await page.evaluate(async ({ quiet, loud, day, night }) => {
    const render = (sound: SoundFrame) => window.gameTest.audio.renderOffline({
      duration: 4, volume: 1, includeSamples: true,
      frames: Array.from({ length: 120 }, (_, index) => ({ time: index / 30, frame: sound })),
    });
    return Promise.all([render(quiet), render(loud), render(day), render(night)]);
  }, {
    quiet: frame({ level: 0, pan: 0 }, { day: 0, night: 0, distance: 0 }),
    loud: frame({ level: 1, pan: 0 }, { day: 0, night: 0, distance: 0 }),
    day: frame({ level: 0, pan: 0 }, { day: 1, night: 0, distance: 1 }),
    night: frame({ level: 0, pan: 0 }, { day: 0, night: 1, distance: 1 }),
  });
  const [quiet, loud, day, night] = results.map((result) => result.channels[0]!);
  expect(loud!.rms).toBeGreaterThan(Math.max(quiet!.rms * 10, 0.001));
  expect(day!.bands.above2000).toBeGreaterThan(0.0001);
  const powerAt = (samples: number[], frequency: number): number => {
    let real = 0; let imaginary = 0;
    for (let index = 0; index < samples.length; index++) {
      const phase = 2 * Math.PI * frequency * index / 48000;
      real += samples[index]! * Math.cos(phase); imaginary += samples[index]! * Math.sin(phase);
    }
    return Math.hypot(real, imaginary) / samples.length;
  };
  const birdBandPower = (samples: number[]): number => Math.hypot(...Array.from({ length: 11 }, (_, index) => powerAt(samples, 2500 + index * 100)));
  expect(birdBandPower(day!.samples!)).toBeGreaterThan(3 * birdBandPower(night!.samples!));
  expect(powerAt(night!.samples!, 4200)).toBeGreaterThan(10 * powerAt(day!.samples!, 4200));
  for (const result of results) for (const channel of result.channels) expect(channel.peak).toBeLessThan(1);
});

test("the live ambience follows the time of day @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(async () => { await window.gameTest.audio.unlock(); window.gameTest.setTimeOfDay(0); });
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.activeVoices().ambience)).toBe("night");
  await page.evaluate(() => window.gameTest.setTimeOfDay(0.5));
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.activeVoices().ambience)).toBe("day");
});
