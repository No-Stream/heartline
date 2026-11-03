import type { TestingReport } from "./coaster-testing";
import type { GuestDetail } from "./guest-panel/protocol";
import type { Facility } from "./facilities";
import type { Camera, WorldPoint } from "./camera";
import type { Pick, WorldSprite } from "./render/index";
import type { CrestBurst } from "./render/effects";

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
