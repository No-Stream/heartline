import type { TestingReport } from "./coaster-testing";
import type { GuestDetail } from "./guest-panel/protocol";
import type { Facility } from "./facilities";
import type { Camera, WorldPoint } from "./camera";
import type { Pick, WorldSprite } from "./render/index";

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
  advanceTicks(count: number): Promise<void>;
  setCamera(camera: Camera): void;
  setSurface(x: number, y: number, corners: [number, number, number, number]): void;
  project(point: WorldPoint): { x: number; y: number };
  pick(x: number, y: number): Pick | null;
  pickSurface(x: number, y: number): Pick | null;
}

declare global {
  interface Window { gameTest: GameTest; workTiming?: { meanMs: number; p95Ms: number; cpuMeanMs: number; frames: number; renderer: string } }
}
