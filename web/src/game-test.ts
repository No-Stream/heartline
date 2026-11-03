import type { Camera, WorldPoint } from "./camera";
import type { Pick } from "./render/index";

export interface GameTest {
  ready: boolean;
  setCamera(camera: Camera): void;
  project(point: WorldPoint): { x: number; y: number };
  pick(x: number, y: number): Pick | null;
}

declare global {
  interface Window { gameTest: GameTest; workTiming?: { meanMs: number; p95Ms: number; cpuMeanMs: number; frames: number; renderer: string } }
}
