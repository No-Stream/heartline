import type { Camera, WorldPoint } from "./camera";
import type { Pick, WorldSprite } from "./render/index";

export interface GameTest {
  ready: boolean;
  guests(): readonly WorldSprite[];
  setCamera(camera: Camera): void;
  setSurface(x: number, y: number, corners: [number, number, number, number]): void;
  project(point: WorldPoint): { x: number; y: number };
  pick(x: number, y: number): Pick | null;
}

declare global {
  interface Window { gameTest: GameTest; workTiming?: { meanMs: number; p95Ms: number; cpuMeanMs: number; frames: number; renderer: string } }
}
