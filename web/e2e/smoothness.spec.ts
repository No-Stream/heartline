import { expect, test } from "@playwright/test";
import type { FrameLogEntry, SnapshotArrivalEntry } from "../src/game-test";

test.use({ viewport: { width: 160, height: 90 } });

type Point3d = { x: number; y: number; z: number };
type TrackProjection = { fraction: number; distance: number };

declare global {
  interface Window {
    smoothnessRails?: number[];
  }
}

function percentile(values: number[], quantile: number): number {
  if (values.length === 0) throw new Error("Cannot summarize an empty sample");
  const sorted = [...values].sort((first, second) => first - second);
  const index = Math.max(0, Math.ceil(quantile * sorted.length) - 1);
  return sorted[index];
}

function distribution(values: number[]): { count: number; median: number; p95: number; max: number } {
  return {
    count: values.length,
    median: percentile(values, 0.5),
    p95: percentile(values, 0.95),
    max: Math.max(...values),
  };
}

function centerlinePoints(rails: number[]): Point3d[] {
  if (rails.length === 0 || rails.length % 38 !== 0) return [];
  const points: Point3d[] = [];
  for (let offset = 0; offset < rails.length; offset += 38) {
    points.push({
      x: (rails[offset + 1] + rails[offset + 20]) / 2,
      y: (rails[offset + 2] + rails[offset + 21]) / 2,
      z: (rails[offset + 3] + rails[offset + 22]) / 2,
    });
  }
  return points.every(({ x, y, z }) => Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(z)) ? points : [];
}

function projectOntoCircuit(position: Point3d, points: Point3d[]): TrackProjection {
  let distanceBeforeSegment = 0;
  const segmentLengths = points.map((point, index) => {
    const next = points[(index + 1) % points.length];
    return Math.hypot(next.x - point.x, next.y - point.y, next.z - point.z);
  });
  const totalLength = segmentLengths.reduce((sum, length) => sum + length, 0);

  let closestDistance = Number.POSITIVE_INFINITY;
  let closestProgress = 0;
  for (let index = 0; index < points.length; index += 1) {
    const start = points[index];
    const end = points[(index + 1) % points.length];
    const direction = { x: end.x - start.x, y: end.y - start.y, z: end.z - start.z };
    const segmentLengthSquared = direction.x ** 2 + direction.y ** 2 + direction.z ** 2;
    const fractionAlongSegment = segmentLengthSquared === 0 ? 0 : Math.max(0, Math.min(1,
      ((position.x - start.x) * direction.x + (position.y - start.y) * direction.y + (position.z - start.z) * direction.z) / segmentLengthSquared));
    const projected = {
      x: start.x + direction.x * fractionAlongSegment,
      y: start.y + direction.y * fractionAlongSegment,
      z: start.z + direction.z * fractionAlongSegment,
    };
    const distance = Math.hypot(position.x - projected.x, position.y - projected.y, position.z - projected.z);
    if (distance < closestDistance) {
      closestDistance = distance;
      closestProgress = distanceBeforeSegment + Math.sqrt(segmentLengthSquared) * fractionAlongSegment;
    }
    distanceBeforeSegment += segmentLengths[index];
  }

  return { fraction: closestProgress / totalLength, distance: closestDistance };
}

function renderTickAt(frames: FrameLogEntry[], time: number): number {
  for (let index = 1; index < frames.length; index += 1) {
    const previous = frames[index - 1];
    const current = frames[index];
    if (previous.now <= time && time <= current.now) {
      const fraction = (time - previous.now) / (current.now - previous.now);
      return previous.renderTick + (current.renderTick - previous.renderTick) * fraction;
    }
  }
  throw new RangeError("Render clock sample does not cover the requested window");
}

function slidingWindowProgressRates(frames: FrameLogEntry[], measurementStart: number): number[] {
  const windowMs = 500;
  const lastFrameTime = frames.at(-1)!.now;
  const rates: number[] = [];
  for (let windowStart = measurementStart; windowStart + windowMs <= lastFrameTime; windowStart += 100) {
    const startTick = renderTickAt(frames, windowStart);
    const endTick = renderTickAt(frames, windowStart + windowMs);
    rates.push((endTick - startTick) * 1000 / (windowMs * 30));
  }
  if (rates.length === 0) throw new Error("No complete 500 ms render clock windows were recorded");
  return rates;
}

function coefficientOfVariation(values: number[]): number {
  if (values.length === 0) throw new Error("Cannot calculate variation for an empty sample");
  const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
  if (mean === 0) throw new Error("Render clock made no progress in the measured windows");
  const variance = values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length;
  return Math.sqrt(variance) / mean;
}

test("demo coaster render progress stays smooth on the production build @slow", async ({ page }) => {
  test.setTimeout(90_000);
  await page.addInitScript(() => {
    const scope = window as Window & { smoothnessRails?: number[] };
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      constructor(scriptURL: string | URL, options?: WorkerOptions) {
        super(scriptURL, options);
        this.addEventListener("message", (event: MessageEvent<{ type?: string; coasterRails?: Float32Array }>) => {
          if (event.data.type === "world" && event.data.coasterRails) scope.smoothnessRails = Array.from(event.data.coasterRails);
        });
      }
    };
  });

  await page.goto("/?frameLog=1");
  await page.waitForFunction(() => window.smoothnessRails !== undefined && window.gameTest.frameLog().length > 0);
  await page.waitForTimeout(3_000);
  const measurementStart = await page.evaluate(() => ({
    now: performance.now(),
    frameCount: window.gameTest.frameLog().length,
    arrivalCount: window.gameTest.snapshotArrivals().length,
  }));
  await page.waitForTimeout(10_000);
  const sample = await page.evaluate((start) => {
    const frames = window.gameTest.frameLog();
    const arrivals = window.gameTest.snapshotArrivals();
    return {
      frames: frames.slice(Math.max(0, start.frameCount - 1)) as FrameLogEntry[],
      arrivals: arrivals.slice(Math.max(0, start.arrivalCount - 1)) as SnapshotArrivalEntry[],
      rails: window.smoothnessRails ?? [],
      durationMs: performance.now() - start.now,
    };
  }, measurementStart);

  const frames = sample.frames;
  const arrivals = sample.arrivals;
  const track = centerlinePoints(sample.rails);
  const inputCounts = {
    durationMs: Math.round(sample.durationMs),
    frames: frames.length,
    arrivals: arrivals.length,
    railRecords: sample.rails.length / 19,
    centerlineSegments: track.length,
  };
  console.log(`SMOOTHNESS_INPUT ${JSON.stringify(inputCounts)}`);
  expect(frames.length).toBeGreaterThan(1);
  expect(arrivals.length).toBeGreaterThan(1);
  expect(track.length).toBeGreaterThan(1);

  const snapshotArrivalIntervalsMs = arrivals.slice(1).map((arrival, index) => arrival.now - arrivals[index].now);
  const snapshotSendIntervalsMs = arrivals.slice(1).map((arrival, index) => arrival.sentAt - arrivals[index].sentAt);
  const frameIntervalsMs = frames.slice(1).map((frame, index) => frame.now - frames[index].now);
  const normalizedProgressRates = frames.slice(1).flatMap((frame, index) => {
    const previous = frames[index];
    const elapsedMs = frame.now - previous.now;
    if (elapsedMs <= 0) throw new RangeError("Frame timestamps must increase");
    return [(frame.renderTick - previous.renderTick) * 1000 / (elapsedMs * 30)];
  });
  const progressWindowRates = slidingWindowProgressRates(frames, measurementStart.now);
  const progressCoefficientOfVariation = coefficientOfVariation(progressWindowRates);
  const movementSteps = frames.slice(1).flatMap((frame, index) => {
    const previous = frames[index];
    const elapsedMs = frame.now - previous.now;
    if (elapsedMs <= 0) throw new RangeError("Frame timestamps must increase");
    const distanceInTiles = Math.hypot(frame.trainX - previous.trainX, frame.trainY - previous.trainY, frame.trainZ - previous.trainZ);
    return [{ distanceInTiles, speedMetresPerSecond: distanceInTiles * 4 * 1000 / elapsedMs }];
  });
  const medianSpeedMetresPerSecond = percentile(movementSteps.map(({ speedMetresPerSecond }) => speedMetresPerSecond).filter((speedMetresPerSecond) => speedMetresPerSecond > 0), 0.5);
  const zeroDisplacementFrames = movementSteps.filter(({ distanceInTiles }) => distanceInTiles === 0).length;
  const trackProgress = frames.map((frame) => projectOntoCircuit({ x: frame.trainX, y: frame.trainY, z: frame.trainZ }, track));
  const lapAdjustedProgressDeltas: number[] = [];
  let lapWraps = 0;
  for (let index = 1; index < trackProgress.length; index += 1) {
    const previous = trackProgress[index - 1].fraction;
    const current = trackProgress[index].fraction;
    let delta = current - previous;
    if (previous > 0.75 && current < 0.25) {
      delta += 1;
      lapWraps += 1;
    }
    lapAdjustedProgressDeltas.push(delta);
  }
  const totalNormalizedTrainProgress = lapAdjustedProgressDeltas.reduce((sum, delta) => sum + delta, 0);
  const backwardsTrainFrames = lapAdjustedProgressDeltas.filter((delta) => delta < 0).length;
  const maximumTrackProjectionDistance = Math.max(...trackProgress.map(({ distance }) => distance));
  const report = {
    durationMs: Math.round(sample.durationMs),
    frameCount: frames.length,
    frameIntervalMs: distribution(frameIntervalsMs),
    snapshotArrivalCount: arrivals.length,
    snapshotArrivalIntervalMs: distribution(snapshotArrivalIntervalsMs),
    snapshotSendIntervalMs: distribution(snapshotSendIntervalsMs),
    renderClockProgress: {
      normalizedRate: distribution(normalizedProgressRates),
      slidingWindowMs: 500,
      slidingWindowStepMs: 100,
      slidingWindowRates: distribution(progressWindowRates),
      slidingWindowCoefficientOfVariation: progressCoefficientOfVariation,
    },
    train: {
      trackSegmentCount: track.length,
      maximumTrackProjectionDistance,
      normalizedProgress: totalNormalizedTrainProgress,
      lapWraps,
      backwardsFrames: backwardsTrainFrames,
      medianSpeedMetresPerSecond,
      zeroDisplacementFrames,
    },
  };
  console.log(`SMOOTHNESS ${JSON.stringify(report)}`);

  expect(sample.durationMs).toBeGreaterThanOrEqual(9_500);
  expect(snapshotSendIntervalsMs.length).toBeGreaterThan(0);
  expect(Math.min(...snapshotSendIntervalsMs)).toBeGreaterThan(0);
  expect(percentile(snapshotSendIntervalsMs, 0.95)).toBeLessThanOrEqual(50);
  expect(Math.max(...snapshotSendIntervalsMs)).toBeLessThanOrEqual(120);
  expect(normalizedProgressRates.length).toBeGreaterThan(0);
  expect(progressCoefficientOfVariation).toBeLessThanOrEqual(0.25);
  expect(totalNormalizedTrainProgress).toBeGreaterThan(0);
  expect(backwardsTrainFrames).toBe(0);
  expect(medianSpeedMetresPerSecond).toBeGreaterThan(1);
  expect(zeroDisplacementFrames).toBe(0);
});
