import { expect, test } from "@playwright/test";
import type { SoundFrame, TrainSound } from "../src/audio/mapping";

const frame = (crowd: SoundFrame["crowd"], ambience: SoundFrame["ambience"], trains: TrainSound[] = []): SoundFrame => ({
  trains, screams: [], crowd, ambience, ui: [], eventsMuted: false,
});
const train = (changes: Partial<TrainSound>): TrainSound => ({ train: 1, pan: 0, audibility: 1, speed: 0, material: "steel", onLift: false, clackRate: 0, launch: false, brake: 0, depart: false, arrive: false, ...changes });

test("crowd murmur follows its level and the ambience is the same wind by day and night @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const results = await page.evaluate(async ({ quiet, loud, day, night }) => {
    const render = (sound: SoundFrame) => window.gameTest.audio.renderOffline({
      duration: 4, volume: 1,
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
  expect(day!.rms).toBeGreaterThan(0.001);
  expect(night!.rms).toBeCloseTo(day!.rms, 6);
  for (const ambience of [day!, night!]) expect(ambience.bands.above2000).toBeLessThan(0.1 * ambience.bands.below200);
  for (const result of results) for (const channel of result.channels) expect(channel.peak).toBeLessThan(1);
});

test("a park with a stopped or cruising train has no narrow-band tone above 2 kHz by day or night @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const scenes: Record<string, SoundFrame> = {};
  for (const [time, ambience] of [["day", { day: 1, night: 0, distance: 1 }], ["night", { day: 0, night: 1, distance: 1 }]] as const) {
    for (const [motion, trainSound] of [["stopped", train({ speed: 0 })], ["cruising steel", train({ speed: 15 })], ["cruising wood", train({ speed: 15, material: "wood" })]] as const) {
      scenes[`${motion} train, ${time}`] = frame({ level: 0.2, pan: 0 }, ambience, [trainSound]);
    }
  }
  const prominence = await page.evaluate(async (sceneFrames) => {
    const result: Record<string, { frequency: number; ratio: number }> = {};
    for (const [name, sound] of Object.entries(sceneFrames)) {
      const rendered = await window.gameTest.audio.renderOffline({ duration: 3, volume: 1, includeSamples: true, frames: Array.from({ length: 90 }, (_, index) => ({ time: index / 30, frame: sound })) });
      const samples = rendered.channels[0]!.samples!.map((sample, index) => sample + rendered.channels[1]!.samples![index]!);
      // Averaged short-time spectrum from 1 s on (after start-up ramps), 11.7 Hz per bin, Hann window.
      const frameLength = 4096; const firstBin = Math.ceil(2000 * frameLength / 48000); const lastBin = Math.floor(12000 * frameLength / 48000);
      const power = new Float64Array(lastBin + 1);
      const fft = (real: Float64Array, imaginary: Float64Array): void => {
        for (let index = 1, reversed = 0; index < real.length; index++) {
          let bit = real.length >> 1;
          for (; reversed & bit; bit >>= 1) reversed ^= bit;
          reversed ^= bit;
          if (index < reversed) { [real[index], real[reversed]] = [real[reversed]!, real[index]!]; [imaginary[index], imaginary[reversed]] = [imaginary[reversed]!, imaginary[index]!]; }
        }
        for (let size = 2; size <= real.length; size <<= 1) {
          const angle = -2 * Math.PI / size;
          for (let start = 0; start < real.length; start += size) {
            for (let offset = 0; offset < size / 2; offset++) {
              const cos = Math.cos(angle * offset); const sin = Math.sin(angle * offset);
              const even = start + offset; const odd = even + size / 2;
              const oddReal = real[odd]! * cos - imaginary[odd]! * sin; const oddImaginary = real[odd]! * sin + imaginary[odd]! * cos;
              real[odd] = real[even]! - oddReal; imaginary[odd] = imaginary[even]! - oddImaginary;
              real[even] = real[even]! + oddReal; imaginary[even] = imaginary[even]! + oddImaginary;
            }
          }
        }
      };
      for (let start = 48000; start + frameLength <= samples.length; start += frameLength / 2) {
        const real = Float64Array.from({ length: frameLength }, (_, index) => samples[start + index]! * (0.5 - 0.5 * Math.cos(2 * Math.PI * index / frameLength)));
        const imaginary = new Float64Array(frameLength);
        fft(real, imaginary);
        for (let bin = firstBin; bin <= lastBin; bin++) power[bin] += real[bin]! ** 2 + imaginary[bin]! ** 2;
      }
      let worst = { frequency: 0, ratio: 0 };
      for (let bin = firstBin + 20; bin <= lastBin - 20; bin++) {
        const neighbours = Array.from(power.slice(bin - 20, bin + 21)).sort((a, b) => a - b);
        const ratio = power[bin]! / Math.max(neighbours[20]!, 1e-18);
        if (ratio > worst.ratio) worst = { frequency: bin * 48000 / frameLength, ratio };
      }
      result[name] = worst;
    }
    return result;
  }, scenes);
  for (const [name, worst] of Object.entries(prominence)) expect(worst.ratio, `${name}: strongest line at ${worst.frequency.toFixed(0)} Hz`).toBeLessThan(10);
});

test("the live ambience follows the time of day @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(async () => { await window.gameTest.audio.unlock(); window.gameTest.setTimeOfDay(0); });
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.activeVoices().ambience)).toBe("night");
  await page.evaluate(() => window.gameTest.setTimeOfDay(0.5));
  await expect.poll(() => page.evaluate(() => window.gameTest.audio.activeVoices().ambience)).toBe("day");
});
