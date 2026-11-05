import { expect, test } from "@playwright/test";
import type { SoundFrame, TrainSound } from "../src/audio/mapping";

const train = (changes: Partial<TrainSound> = {}): TrainSound => ({ train: 1, pan: 0, audibility: 1, speed: 0, material: "steel", onLift: false, clackRate: 0, launch: false, brake: 0, depart: false, arrive: false, ...changes });
const frame = (sound: TrainSound): SoundFrame => ({ trains: [sound], screams: [], crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: [], eventsMuted: false });

test("train sounds respond to speed, material and operating events @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const render = (sound: TrainSound) => page.evaluate((f) => window.gameTest.audio.renderOffline({ duration: 2, volume: 1, frames: Array.from({ length: 60 }, (_, i) => ({ time: i / 30, frame: i === 0 ? f : { ...f, trains: f.trains.map((t) => ({ ...t, launch: false, depart: false, arrive: false })) } })) }), frame(sound));
  const slow = (await render(train({ speed: 5 }))).channels[0]!;
  const fast = (await render(train({ speed: 25 }))).channels[0]!;
  const wood = (await render(train({ speed: 25, material: "wood" }))).channels[0]!;
  const launch = (await render(train({ launch: true }))).channels[0]!;
  const brake = (await render(train({ brake: 1 }))).channels[0]!;
  const station = (await render(train({ depart: true }))).channels[0]!;
  const lift = (await render(train({ speed: 2, onLift: true, clackRate: 5 }))).channels[0]!;
  expect(fast.rms).toBeGreaterThan(4 * slow.rms);
  expect(wood.bands.below200).toBeGreaterThan(fast.bands.below200);
  expect(launch.bands.above2000).toBeGreaterThan(0.001);
  expect(brake.bands.above2000).toBeGreaterThan(0.001);
  expect(station.rms).toBeGreaterThan(0.001);
  expect(lift.bands.above2000).toBeGreaterThan(slow.bands.above2000);
  for (const result of [slow, fast, wood, launch, brake, station, lift]) expect(result.peak).toBeLessThan(1);
});

test("lift clacks and station chime have the expected timing and pitch @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const measure = async (sound: TrainSound) => page.evaluate(async (f) => {
    const result = await window.gameTest.audio.renderOffline({ duration: 2, volume: 1, includeSamples: true, frames: Array.from({ length: 60 }, (_, index) => ({ time: index / 30, frame: index === 0 ? f : { ...f, trains: f.trains.map((t) => ({ ...t, depart: false })) } })) });
    const samples = result.channels[0]!.samples!;
    let low = 0; const envelopes: number[] = [];
    for (let start = 0; start < samples.length; start += 480) {
      let energy = 0;
      for (let i = start; i < start + 480; i++) { low += 0.2 * (samples[i]! - low); energy += (samples[i]! - low) ** 2; }
      envelopes.push(Math.sqrt(energy / 480));
    }
    const peaks = envelopes.filter((value, i) => i > 0 && i < envelopes.length - 1 && value > 0.004 && value > envelopes[i - 1]! && value > envelopes[i + 1]!).length;
    const amplitude = (frequency: number, start: number): number => {
      let real = 0; let imaginary = 0;
      for (let i = Math.round(start * 48000); i < Math.round((start + 0.08) * 48000); i++) { real += samples[i]! * Math.cos(2 * Math.PI * frequency * i / 48000); imaginary += samples[i]! * Math.sin(2 * Math.PI * frequency * i / 48000); }
      return Math.hypot(real, imaginary);
    };
    return { peaks, first: amplitude(880, 0.02), second: amplitude(1175, 0.22), wrongFirst: amplitude(1175, 0.02), wrongSecond: amplitude(880, 0.22) };
  }, frame(sound));
  const lift = await measure(train({ speed: 2, onLift: true, clackRate: 5 }));
  expect(lift.peaks).toBeGreaterThanOrEqual(9);
  expect(lift.peaks).toBeLessThanOrEqual(11);
  const chime = await measure(train({ depart: true }));
  expect(chime.first).toBeGreaterThan(4 * chime.wrongFirst);
  expect(chime.second).toBeGreaterThan(2 * chime.wrongSecond);
});

test("muting events and releasing a voice stop its scheduled sounds @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const eventFrame = frame(train({ launch: true, depart: true }));
  for (const nextFrame of [{ ...frame(train()), eventsMuted: true }, { ...eventFrame, trains: [] }]) {
    const result = await page.evaluate((frames) => window.gameTest.audio.renderOffline({ duration: 1, volume: 1, includeSamples: true, frames }), [{ time: 0, frame: eventFrame }, { time: 0.2, frame: nextFrame }]);
    const samples = result.channels[0]!.samples!;
    const rms = (start: number, end: number) => Math.sqrt(samples.slice(start * 48000, end * 48000).reduce((sum, sample) => sum + sample * sample, 0) / ((end - start) * 48000));
    expect(rms(0.05, 0.15)).toBeGreaterThan(0.005);
    expect(rms(0.6, 0.9)).toBeLessThan(0.0001);
  }
});
