import { projectWorld, type Camera, type Viewport, type WorldPoint } from "../camera";
import type { CoasterSnapshot } from "../protocol";
import type { TrainLayout } from "../train-layout";
import type { CrestBurst } from "../render/effects";

export interface TrainSound {
  train: number; pan: number; audibility: number; speed: number; material: "steel" | "wood" | "hybrid";
  onLift: boolean; clackRate: number; launch: boolean; brake: number; depart: boolean; arrive: boolean;
}
export interface ScreamEvent { train: number; pan: number; audibility: number; intensity: number }
export type UiEvent = "place" | "undo" | "error" | "finish";
export interface SoundFrame {
  trains: TrainSound[];
  screams: ScreamEvent[];
  crowd: { level: number; pan: number };
  ambience: { day: number; night: number; distance: number };
  ui: UiEvent[];
  eventsMuted: boolean;
}
export interface MappingInput {
  snapshot: CoasterSnapshot | undefined; interpolation: number; camera: Camera; viewport: Viewport;
  dayPhase: number; gameSpeed: number; demoTrain: TrainLayout | null; constructionTrain: TrainLayout | null;
  crestBursts: readonly CrestBurst[];
}
export function spatialise(camera: Camera, viewport: Viewport, point: WorldPoint): { pan: number; audibility: number; distance: number } {
  const projected = projectWorld(camera, viewport, point);
  const dx = projected.x - viewport.width / 2;
  const dy = projected.y - viewport.height / 2;
  const distance = Math.hypot(dx, dy) / camera.zoom;
  const pan = Math.max(-1, Math.min(1, dx / (viewport.width / 2))) * 0.8;
  const zoomGain = Math.max(0.35, Math.min(1.2, camera.zoom / 2));
  return { pan, audibility: zoomGain / (1 + (distance / 220) ** 2), distance };
}
export function soundFrame(input: MappingInput): SoundFrame {
  return { trains: [], screams: [], crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: [], eventsMuted: input.gameSpeed > 4 };
}
