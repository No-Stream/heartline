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
  // Short frames give each DFT bin about 23 Hz of bandwidth, so gliding bird notes count fully and the cricket gating sidebands do not line up with the bins.
  const birdBandEnergy = (result: { channels: { samples?: number[] }[] }): number => {
    const samples = result.channels[0]!.samples!.map((sample, index) => sample + result.channels[1]!.samples![index]!);
    const frameLength = 2048; let energy = 0;
    for (let start = 0; start + frameLength <= samples.length; start += frameLength) {
      for (let bin = Math.ceil(2000 * frameLength / 48000); bin <= Math.floor(3600 * frameLength / 48000); bin++) {
        let real = 0; let imaginary = 0;
        for (let index = 0; index < frameLength; index++) { const phase = 2 * Math.PI * bin * index / frameLength; real += samples[start + index]! * Math.cos(phase); imaginary += samples[start + index]! * Math.sin(phase); }
        energy += real * real + imaginary * imaginary;
      }
    }
    return energy;
  };
  expect(birdBandEnergy(results[2]!)).toBeGreaterThan(3 * birdBandEnergy(results[3]!));
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

test("day birds sing short, quiet phrases that vary in pitch and rhythm @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const birds = frame({ level: 0, pan: 0 }, { day: 1, night: 0, distance: 0 });
  const result = await page.evaluate((sound) => window.gameTest.audio.renderOffline({
    duration: 4, volume: 1, includeSamples: true,
    frames: Array.from({ length: 120 }, (_, index) => ({ time: index / 30, frame: sound })),
  }), birds);
  const [left, right] = result.channels;
  const mono = left!.samples!.map((sample, index) => sample + right!.samples![index]!);
  const frameLength = 240;
  const frameRms = Array.from({ length: Math.floor(mono.length / frameLength) }, (_, index) => Math.sqrt(mono.slice(index * frameLength, (index + 1) * frameLength).reduce((sum, sample) => sum + sample * sample, 0) / frameLength));
  const threshold = 0.1 * Math.max(...frameRms);
  const notes: { start: number; frequency: number }[] = [];
  for (let index = 0; index < frameRms.length;) {
    if (frameRms[index]! < threshold) { index++; continue; }
    const first = index;
    while (index < frameRms.length && frameRms[index]! >= threshold) index++;
    const samples = mono.slice(first * frameLength, index * frameLength);
    let crossings = 0;
    for (let sample = 1; sample < samples.length; sample++) if ((samples[sample - 1]! < 0) !== (samples[sample]! < 0)) crossings++;
    if (index - first >= 3) notes.push({ start: first * frameLength / 48000, frequency: crossings * 48000 / (2 * samples.length) });
  }
  const frequencies = notes.map((note) => note.frequency);
  const intervals = notes.slice(1).map((note, index) => note.start - notes[index]!.start);
  const mean = intervals.reduce((sum, value) => sum + value, 0) / intervals.length;
  const spread = Math.sqrt(intervals.reduce((sum, value) => sum + (value - mean) ** 2, 0) / intervals.length) / mean;
  expect(notes.length).toBeGreaterThanOrEqual(5);
  expect(Math.max(...frequencies) / Math.min(...frequencies)).toBeGreaterThan(1.2);
  expect(spread).toBeGreaterThan(0.25);
  for (const channel of result.channels) { expect(channel.peak).toBeLessThan(0.1); expect(channel.peak).toBeGreaterThan(0.002); }
});
