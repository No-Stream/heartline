import { projectWorld, type Camera, type Viewport, type WorldPoint } from "../camera";
import type { CoasterSnapshot } from "../protocol";
import type { TrainLayout } from "../train-layout";
import type { CrestBurst } from "../render/effects";
import { BRIDGE_LAYOUTS } from "../bridge-layouts";
import { parseTrainStates, type TrainState } from "../train-states";

export type TrackMaterial = TrainLayout["trackMaterial"];
export interface TrainSound {
  train: number; pan: number; audibility: number; distance?: number; speed: number; material: TrackMaterial;
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
  paused?: boolean;
}
export interface MappingInput {
  snapshot: CoasterSnapshot | undefined; interpolation: number; camera: Camera; viewport: Viewport;
  dayPhase: number; gameSpeed: number; demoTrain: TrainLayout | null; constructionTrain: TrainLayout | null;
  crestBursts: readonly CrestBurst[]; paused: boolean; timeSeconds: number;
  /** Builder and tool events collected since the previous frame, already debounced. */
  ui: readonly UiEvent[];
}
export function trainMaterial(layout: TrainLayout): TrackMaterial { return layout.trackMaterial; }
export function spatialise(camera: Camera, viewport: Viewport, point: WorldPoint): { pan: number; audibility: number; distance: number } {
  const projected = projectWorld(camera, viewport, point);
  const dx = projected.x - viewport.width / 2;
  const dy = projected.y - viewport.height / 2;
  const distance = Math.hypot(dx, dy) / camera.zoom;
  const pan = Math.max(-1, Math.min(1, dx / (viewport.width / 2))) * 0.8;
  const zoomGain = Math.max(0.35, Math.min(1.2, camera.zoom / 2));
  return { pan, audibility: zoomGain / (1 + (distance / 220) ** 2), distance };
}
export const SOUND_TUNING = {
  chainLinkSpacing: 0.4, maxEventSpeed: 4, fastForwardSpeed: 5,
  stopSpeed: 0.1, brakeSpeed: 3, brakeDelta: 8,
  airtimeG: 0.2, highG: 3, lateralG: 1.2, screamCooldown: 2.5,
  crestDistance: 2, crowdSaturation: 200,
} as const;
interface TrainMemory { state: TrainState; screamTime: number; brake: number }
export interface MappingMemory { trains: Map<number, TrainMemory>; tick: number; crowd: SoundFrame["crowd"]; crowdFrame: number }
const clamp01 = (value: number): number => Math.max(0, Math.min(1, value));
function forceEdge(old: TrainState, state: TrainState): boolean {
  return (old.verticalG >= SOUND_TUNING.airtimeG && state.verticalG < SOUND_TUNING.airtimeG)
    || (old.verticalG <= SOUND_TUNING.highG && state.verticalG > SOUND_TUNING.highG)
    || (Math.abs(old.lateralG) <= SOUND_TUNING.lateralG && Math.abs(state.lateralG) > SOUND_TUNING.lateralG);
}
function smoothstep(start: number, end: number, value: number): number {
  const fraction = clamp01((value - start) / (end - start));
  return fraction * fraction * (3 - 2 * fraction);
}
export function soundFrame(input: MappingInput, previous: MappingMemory = { trains: new Map(), tick: -1, crowd: { level: 0, pan: 0 }, crowdFrame: 0 }): { frame: SoundFrame; memory: MappingMemory } {
  const eventsMuted = input.gameSpeed > SOUND_TUNING.maxEventSpeed || input.paused;
  const frame: SoundFrame = { trains: [], screams: [], crowd: { level: 0, pan: 0 }, ambience: { day: 0, night: 0, distance: 0 }, ui: input.paused ? [] : [...input.ui], eventsMuted, paused: input.paused };
  const memory: MappingMemory = { trains: new Map(), tick: input.snapshot?.tickCount ?? -1, crowd: previous.crowd, crowdFrame: previous.crowdFrame + 1 };
  const day = smoothstep(0.22, 0.28, input.dayPhase) * (1 - smoothstep(0.72, 0.78, input.dayPhase));
  const zoomGain = Math.max(0.35, Math.min(1.2, input.camera.zoom / 2));
  frame.ambience = { day, night: 1 - day, distance: 0.6 + 0.4 * (1 - zoomGain) };
  const snapshot = input.snapshot;
  if (!snapshot) return { frame, memory };
  if (previous.crowdFrame % 4 === 0) {
    let weightedCount = 0, weightedPan = 0;
    const ridingGuests = new Set<number>();
    for (let offset = 0; offset < snapshot.constructionRiders.length; offset += BRIDGE_LAYOUTS.RIDER.length) ridingGuests.add(snapshot.constructionRiders[offset + 2]);
    for (let offset = 0; offset < snapshot.guests.length; offset += BRIDGE_LAYOUTS.GUEST.length) {
      if (ridingGuests.has(snapshot.guestIds[offset / BRIDGE_LAYOUTS.GUEST.length])) continue;
      const position = spatialise(input.camera, input.viewport, { x: snapshot.guests[offset], y: snapshot.guests[offset + 1], z: snapshot.guests[offset + 2] });
      const weight = 1 / (1 + (position.distance / 220) ** 2);
      weightedCount += weight; weightedPan += weight * position.pan;
    }
    memory.crowd = { level: 0.25 * Math.min(1, Math.log1p(weightedCount) / Math.log1p(SOUND_TUNING.crowdSaturation)), pan: weightedCount ? weightedPan / weightedCount : 0 };
  }
  frame.crowd = memory.crowd;
  const coasters = [
    { layout: input.demoTrain, states: snapshot.trainStates, poses: snapshot.trainPoses, coaster: 0 },
    { layout: input.constructionTrain, states: snapshot.constructionTrainStates, poses: snapshot.constructionTrainPoses, coaster: 1 },
  ];
  for (const { layout, states, poses, coaster } of coasters) {
    if (!layout) continue;
    for (const state of parseTrainStates(states)) {
      const train = state.trainIndex * 2 + coaster;
      const old = previous.trains.get(train);
      let offset = 0;
      while (offset < poses.length && poses[offset] !== state.trainIndex * layout.carsPerTrain) offset += BRIDGE_LAYOUTS.TRAIN_POSE.length;
      if (offset === poses.length) continue;
      const point = { x: poses[offset + 1], y: poses[offset + 2], z: poses[offset + 3] };
      const position = spatialise(input.camera, input.viewport, point);
      const crest = input.crestBursts.filter((burst) => burst.tick > previous.tick && burst.tick <= snapshot.tickCount && Math.hypot(burst.x - point.x, burst.y - point.y, burst.z - point.z) < SOUND_TUNING.crestDistance).at(-1);
      let screamTime = old?.screamTime ?? -Infinity;
      if (!eventsMuted && state.occupied && old && input.timeSeconds - screamTime >= SOUND_TUNING.screamCooldown && (forceEdge(old.state, state) || crest)) {
        frame.screams.push({ train, pan: position.pan, audibility: position.audibility, intensity: clamp01(Math.max((SOUND_TUNING.airtimeG - state.verticalG), (state.verticalG - SOUND_TUNING.highG) / 2, (Math.abs(state.lateralG) - SOUND_TUNING.lateralG) / 2, crest ? 0.5 : 0)) });
        screamTime = input.timeSeconds;
      }
      const delta = old ? state.speed - old.state.speed : 0;
      const braking = old && (state.pieceKind === 3 || state.pieceKind === 4 || old.state.pieceKind === 3 || old.state.pieceKind === 4) && old.state.speed > SOUND_TUNING.brakeSpeed;
      const brake = snapshot.tickCount === previous.tick ? old?.brake ?? 0 : braking ? clamp01(-delta / SOUND_TUNING.brakeDelta) : 0;
      memory.trains.set(train, { state, screamTime, brake });
      if (input.paused) continue;
      frame.trains.push({ train, pan: position.pan, audibility: position.audibility, distance: position.distance, speed: eventsMuted ? SOUND_TUNING.fastForwardSpeed : state.speed, material: trainMaterial(layout), onLift: state.pieceKind === 1, clackRate: !eventsMuted && state.pieceKind === 1 && state.speed > SOUND_TUNING.stopSpeed ? state.speed * input.gameSpeed / SOUND_TUNING.chainLinkSpacing : 0, launch: !eventsMuted && !!old && state.pieceKind === 2 && old.state.pieceKind !== 2 && delta > 0, brake: eventsMuted ? 0 : brake, arrive: !eventsMuted && !!old && state.pieceKind === 4 && old.state.speed > SOUND_TUNING.stopSpeed && state.speed <= SOUND_TUNING.stopSpeed, depart: !eventsMuted && !!old && state.pieceKind === 4 && old.state.speed <= SOUND_TUNING.stopSpeed && state.speed > SOUND_TUNING.stopSpeed });
    }
  }
  frame.trains.sort((a, b) => b.audibility - a.audibility);
  frame.trains = frame.trains.slice(0, 4);
  return { frame, memory };
}
