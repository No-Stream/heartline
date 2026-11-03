import type { TestingReport } from "./coaster-testing";
import type { GuestDetail } from "./guest-panel/protocol";
import type { Facility } from "./facilities";
import type { Camera, WorldPoint } from "./camera";
import type { Pick, WorldSprite } from "./render/index";
import type { CrestBurst } from "./render/effects";

export function constructionRiderPositions(poses: Float32Array, riders: Uint32Array): { id: string; car: number; seat: number; x: number; y: number; z: number }[] {
  const poseOffsets = new Map<number, number>();
  for (let offset = 0; offset < poses.length; offset += 11) poseOffsets.set(poses[offset], offset);
  const result: { id: string; car: number; seat: number; x: number; y: number; z: number }[] = [];
  for (let offset = 0; offset < riders.length; offset += 3) {
    const car = riders[offset];
    const poseOffset = poseOffsets.get(car);
    if (poseOffset === undefined) throw new Error("Missing rider car pose");
    result.push({ car, seat: riders[offset + 1], id: `guest-${riders[offset + 2]}`, x: poses[poseOffset + 1], y: poses[poseOffset + 2], z: poses[poseOffset + 3] });
  }
  return result;
}

export interface GameTest {
  ready: boolean;
  guests(): readonly WorldSprite[];
  riders(): readonly { id: string; car: number; seat: number; x: number; y: number; z: number }[];
  facilities(): readonly Facility[];
  paths(): { x: number; y: number; z: number; slope: number; kind: number; style: number }[];
  carvedSpans(): readonly [number, number, number, number][];
  requestGuestDetail(id: string): void;
  guestDetail(): GuestDetail | null;
  testingReport(): TestingReport | null;
  constructionTrainPoses(): readonly number[];
  advanceTicks(count: number): Promise<void>;
  setCamera(camera: Camera): void;
  setSurface(x: number, y: number, corners: [number, number, number, number]): void;
  project(point: WorldPoint): { x: number; y: number };
  pick(x: number, y: number): Pick | null;
  pickSurface(x: number, y: number): Pick | null;
  /** Sets the time of day, from 0 at midnight through 0.5 at noon. */
  setTimeOfDay(phase: number): void;
  timeOfDay(): number;
  /** Confetti bursts trains have thrown at lift hill crests since the page loaded, at their crest position in tiles. */
  crestBursts(): readonly CrestBurst[];
}

declare global {
  interface Window { gameTest: GameTest; workTiming?: { meanMs: number; p95Ms: number; cpuMeanMs: number; frames: number; renderer: string } }
}
