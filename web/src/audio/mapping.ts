import { projectWorld, type Camera, type Viewport, type WorldPoint } from "../camera";
import type { CoasterSnapshot } from "../protocol";
import type { TrainLayout } from "../train-layout";
import type { CrestBurst } from "../render/effects";

export type TrackMaterial = "steel" | "wood" | "hybrid";
export interface TrainSound {
  train: number; pan: number; audibility: number; speed: number; material: TrackMaterial;
  onLift: boolean; clackRate: number; launch: boolean; brake: number; depart: boolean; arrive: boolean;
}
export interface ScreamEvent { train: number; pan: number; audibility: number; intensity: number }
/** `place` is a coaster track piece, `build` a path, scenery or facility, `undo` an undo or removal. */
export type UiSound = "place" | "build" | "undo" | "error" | "finish";
export type UiEvent = { readonly sound: "place"; readonly material: TrackMaterial } | { readonly sound: Exclude<UiSound, "place"> };
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
  /** Builder and tool events collected since the previous frame, already debounced. */
  ui: readonly UiEvent[];
}
const TRAIN_MATERIALS: Record<TrainLayout["model"], TrackMaterial> = { velocity_viper: "steel", wooden_twister: "wood", hybrid: "hybrid" };
export function trainMaterial(layout: TrainLayout): TrackMaterial { return TRAIN_MATERIALS[layout.model]; }
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
  return { trains: [], screams: [], crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: [...input.ui], eventsMuted: input.gameSpeed > 4 };
}
