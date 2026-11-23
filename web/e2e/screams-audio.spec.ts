import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import type { SoundFrame } from "../src/audio/mapping";
import { parseReplaySession } from "../src/session-replay";
import { flattenTerrainTiles, gotoGame, layPath, layQueue, openRide, setGameSpeed } from "./player/driver";

const frame = (screams: SoundFrame["screams"] = []): SoundFrame => ({ trains: [], screams, crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: [], eventsMuted: false });

test("rider screams play the CC0 recordings on time, broadband, and stop when events are muted @fast", async ({ page }) => {
  const recordings = new Map<string, number>();
  page.on("response", (response) => { if (response.url().endsWith(".mp3")) recordings.set(response.url(), response.status()); });
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const scream = frame([{ train: "0:0", pan: 0, audibility: 1, intensity: 1 }]);
  const results = await page.evaluate(async ({ scream, quiet }) => {
    const render = (frames: { time: number; frame: SoundFrame }[]) => window.gameTest.audio.renderOffline({ duration: 3, volume: 1, includeSamples: true, frames });
    return Promise.all([render([{ time: 0, frame: quiet }, { time: 0.2, frame: scream }]), render([{ time: 0, frame: quiet }]), render([{ time: 0, frame: scream }, { time: 0.2, frame: { ...quiet, eventsMuted: true } }])]);
  }, { scream, quiet: frame() });
  expect([...recordings.values()]).toEqual([200, 200, 200, 200, 200]);
  const [active, silent, muted] = results.map((result) => result.channels[0]!);
  const rms = (samples: number[], start: number, end: number): number => {
    const window = samples.slice(Math.round(start * 48000), Math.round(end * 48000));
    return Math.sqrt(window.reduce((sum, sample) => sum + sample * sample, 0) / window.length);
  };
  expect(rms(active!.samples!, 0, 0.2)).toBeLessThan(0.0001);
  expect(rms(active!.samples!, 0.25, 1.5)).toBeGreaterThan(0.005);
  expect(active!.bands.from200To2000).toBeGreaterThan(silent!.bands.from200To2000 + 0.005);
  expect(active!.bands.above2000).toBeGreaterThan(0.2 * active!.bands.from200To2000);
  expect(active!.peak).toBeLessThan(1);
  expect(rms(muted!.samples!, 0.5, 0.9)).toBeLessThan(0.0001);
});

test("simultaneous drops occupy separate groups and a suppressed repeat adds none @fast", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const drops = frame([
    { train: "17:0", pan: -0.4, audibility: 1, intensity: 0.8 },
    { train: "17:1", pan: 0.4, audibility: 1, intensity: 0.8 },
  ]);
  const result = await page.evaluate(({ drops, quiet }) => window.gameTest.audio.renderOffline({ duration: 3, includeVoiceTimeline: true, frames: [
    { time: 0, frame: quiet },
    { time: 0.1, frame: drops },
    { time: 0.3, frame: quiet },
    { time: 2.9, frame: quiet },
  ] }), { drops, quiet: frame() });
  expect(result.voiceTimeline!.map((voices) => voices.screams)).toEqual([0, 2, 2, 0]);
});

test("guests riding an open coaster produce a live scream @slow", async ({ page }) => {
  test.setTimeout(150_000);
  const session = parseReplaySession(JSON.parse(readFileSync(new URL("./sessions/timber-tumbler-built.json", import.meta.url), "utf8")) as unknown);
  const params = new URLSearchParams({ test: "1", manualBlockAnalysis: "1", park: session.park, seed: session.seed, agents: String(session.guestCount) });
  await page.setViewportSize({ width: 1280, height: 720 });
  await gotoGame(page, `/?${params}`);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate((recordedSession) => window.gameTest.replaySession(recordedSession), session);
  await setGameSpeed(page, 1);
  await flattenTerrainTiles(page, [
    ...Array.from({ length: 6 }, (_, index) => 255 - index).flatMap((y, index) => [238, 244].map((x) => ({ x, y, height: Math.min(12, 8 + index) }))),
    { x: 239, y: 250, height: 12 }, { x: 243, y: 250, height: 12 },
  ]);
  await setGameSpeed(page, 16);
  await page.setViewportSize({ width: 1280, height: 720 });
  await layPath(page, { x: 218, y: 256, z: 2 }, { x: 244, y: 256, z: 2 });
  await layQueue(page, { x: 243, y: 250, z: 3 }, { x: 244, y: 256, z: 2 });
  await layPath(page, { x: 239, y: 250, z: 3 }, { x: 238, y: 256, z: 2 });
  await openRide(page);
  await page.evaluate(() => window.gameTest.audio.unlock());
  await setGameSpeed(page, 4);
  await expect.poll(() => page.evaluate(() => window.gameTest.riders().length > 0 && window.gameTest.audio.activeVoices().screams > 0), { timeout: 90_000, intervals: [50] }).toBe(true);
});
