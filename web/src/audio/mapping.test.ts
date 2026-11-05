import { expect, it } from "vitest";
import { soundFrame, spatialise, type MappingInput, type MappingMemory } from "./mapping";
import type { CoasterSnapshot } from "../protocol";

const camera = { centerX: 0, centerY: 0, zoom: 2, rotation: 0 as const };
const viewport = { width: 800, height: 600 };
const layout = { model: "velocity_viper" as const, carsPerTrain: 2, rowsPerCar: 1, seatsPerRow: 2, lengthMetres: 4 };
function input(state: number[], tick = 0): MappingInput {
  const snapshot: CoasterSnapshot = { tickCount: tick, guests: new Float32Array(), guestIds: new Uint32Array(), appearances: new Uint8Array(), trainPoses: new Float32Array([0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0]), constructionTrainPoses: new Float32Array(), trainStates: new Float32Array(state), constructionTrainStates: new Float32Array(), constructionRiders: new Uint32Array() };
  return { snapshot, interpolation: 1, camera, viewport, dayPhase: 0.5, gameSpeed: 1, demoTrain: layout, constructionTrain: null, crestBursts: [], ui: [], paused: false, timeSeconds: tick / 200 };
}
function next(value: MappingInput, memory?: MappingMemory) { return soundFrame(value, memory); }

it("spatialises points in camera screen space and attenuates distant sources", () => {
  const centre = spatialise(camera, viewport, { x: 0, y: 0, z: 0 });
  const right = spatialise(camera, viewport, { x: 10, y: 0, z: 0 });
  expect(centre.pan).toBe(0); expect(centre.audibility).toBe(1);
  expect(right.pan).toBeGreaterThan(0); expect(right.audibility).toBeLessThan(centre.audibility);
});
it("maps train speed, lift, launch, brakes and station edges", () => {
  let result = next(input([0, 2, 1, 1, 0, 1]));
  expect(result.frame.trains[0]).toMatchObject({ speed: 2, distance: 0, onLift: true, clackRate: 5 });
  result = next({ ...input([0, 4, 2, 1, 0, 1], 10), gameSpeed: 4 }, result.memory);
  expect(result.frame.trains[0].launch).toBe(true);
  result = next(input([0, 8, 3, 1, 0, 1], 20), result.memory);
  result = next(input([0, 4, 0, 1, 0, 1], 30), result.memory);
  expect(result.frame.trains[0].brake).toBeCloseTo(0.5);
  result = next(input([0, 0, 4, 1, 0, 1], 40), result.memory);
  expect(result.frame.trains[0].arrive).toBe(true);
  result = next(input([0, 1, 4, 1, 0, 1], 50), result.memory);
  expect(result.frame.trains[0].depart).toBe(true);
});
it("triggers screams on force edges and respects cooldown and occupancy", () => {
  let result = next(input([0, 5, 0, 1, 0, 1]));
  result = next(input([0, 5, 0, 0, 0, 1], 10), result.memory);
  expect(result.frame.screams).toHaveLength(1);
  result = next(input([0, 5, 0, 1, 0, 1], 20), result.memory);
  result = next(input([0, 5, 0, 4, 0, 1], 30), result.memory);
  expect(result.frame.screams).toHaveLength(0);
  result = next(input([0, 5, 0, 1, 0, 1], 600), result.memory);
  result = next(input([0, 5, 0, 1, 2, 1], 610), result.memory);
  expect(result.frame.screams).toHaveLength(1);
  result = next(input([0, 5, 0, 1, 0, 0], 1200), result.memory);
  result = next(input([0, 5, 0, 0, 0, 0], 1210), result.memory);
  expect(result.frame.screams).toHaveLength(0);
});
it("maps crowd and day/night ambience and suppresses events during fast-forward and pause", () => {
  const value = input([0, 2, 1, 1, 0, 1]);
  value.snapshot = { ...value.snapshot!, guests: new Float32Array([5, 0, 0, 0, 0, 0, 0]) };
  let result = next(value);
  expect(result.frame.crowd.level).toBeGreaterThan(0); expect(result.frame.crowd.pan).toBeGreaterThan(0);
  expect(result.frame.ambience.day).toBe(1); expect(result.frame.ambience.night).toBe(0);
  for (const gameSpeed of [8, 16]) {
    result = next({ ...value, gameSpeed, dayPhase: 0, ui: [{ sound: "undo" }] }, result.memory);
    expect(result.frame.eventsMuted).toBe(true); expect(result.frame.trains[0].clackRate).toBe(0);
    expect(result.frame.screams).toEqual([]); expect(result.frame.ui).toEqual([{ sound: "undo" }]);
    expect(result.frame.ambience.night).toBe(1);
  }
  result = next({ ...value, paused: true, ui: [{ sound: "undo" }] }, result.memory);
  expect(result.frame.trains).toEqual([]); expect(result.frame.screams).toEqual([]); expect(result.frame.ui).toEqual([]);
});

it("detects independent force edges and crest events", () => {
  let result = next(input([0, 5, 0, 0, 0, 1]));
  result = next(input([0, 5, 0, 0, 2, 1], 600), result.memory);
  expect(result.frame.screams).toHaveLength(1);
  result = next(input([0, 5, 0, 1, 0, 1], 1200), result.memory);
  result = next({ ...input([0, 5, 0, 1, 0, 1], 1210), crestBursts: [{ x: 0, y: 0, z: 0, tick: 1210 }] }, result.memory);
  expect(result.frame.screams).toHaveLength(1);
  result = next({ ...input([0, 5, 0, 1, 0, 1], 1210), crestBursts: [{ x: 0, y: 0, z: 0, tick: 1210 }] }, result.memory);
  expect(result.frame.screams).toHaveLength(0);
});
it("retains braking between snapshots and scales lift clacks at four times speed", () => {
  let result = next(input([0, 10, 3, 1, 0, 1], 10));
  result = next(input([0, 6, 3, 1, 0, 1], 20), result.memory);
  expect(result.frame.trains[0].brake).toBe(0.5);
  result = next(input([0, 6, 3, 1, 0, 1], 20), result.memory);
  expect(result.frame.trains[0].brake).toBe(0.5);
  result = next({ ...input([0, 2, 1, 1, 0, 1], 30), gameSpeed: 4 }, result.memory);
  expect(result.frame.trains[0].clackRate).toBe(20);
});
it("excludes riders from crowd and caches the crowd for four frames", () => {
  const value = input([]);
  value.snapshot = { ...value.snapshot!, guestIds: new Uint32Array([1, 2]), guests: new Float32Array([0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0]), constructionRiders: new Uint32Array([0, 0, 1]) };
  let result = next(value);
  expect(result.frame.crowd.pan).toBeGreaterThan(0);
  const crowd = result.frame.crowd;
  value.snapshot = { ...value.snapshot, guests: new Float32Array(), guestIds: new Uint32Array() };
  for (let index = 0; index < 3; index++) { result = next(value, result.memory); expect(result.frame.crowd).toEqual(crowd); }
  result = next(value, result.memory);
  expect(result.frame.crowd.level).toBe(0);
});
it("limits train entries to the four most audible and separates coaster identities", () => {
  const value = input([0, 5, 0, 1, 0, 0, 1, 5, 0, 1, 0, 0, 2, 5, 0, 1, 0, 0, 3, 5, 0, 1, 0, 0]);
  value.snapshot = { ...value.snapshot!, trainPoses: new Float32Array([0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 2, 5, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 10, 0, 0, 1, 0, 0, 0, 0, 1, 0, 6, 15, 0, 0, 1, 0, 0, 0, 0, 1, 0]), constructionTrainStates: new Float32Array([0, 5, 0, 1, 0, 0]), constructionTrainPoses: new Float32Array([0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0]) };
  value.constructionTrain = layout;
  const result = next(value);
  expect(result.frame.trains).toHaveLength(4);
  expect(result.frame.trains.map((train) => train.train)).toEqual([0, 1, 2, 4]);
  expect(result.frame.trains[0].audibility).toBeGreaterThan(0.5);
});

it("triggers two trains together and suppresses a repeat during only that train's cooldown", () => {
  const twoTrains = (firstG: number, secondG: number, tick: number): MappingInput => {
    const value = input([0, 5, 0, firstG, 0, 1, 1, 5, 0, secondG, 0, 1], tick);
    value.snapshot = { ...value.snapshot!, trainPoses: new Float32Array([0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 2, 5, 0, 0, 1, 0, 0, 0, 0, 1, 0]) };
    return value;
  };
  let result = next(twoTrains(1, 1, 0));
  result = next(twoTrains(0, 0, 10), result.memory);
  expect(result.frame.screams.map((event) => event.train)).toEqual([0, 2]);
  result = next(twoTrains(1, 1, 20), result.memory);
  result = next(twoTrains(0, 1, 30), result.memory);
  expect(result.frame.screams).toEqual([]);
  result = next(twoTrains(1, 1, 600), result.memory);
  result = next(twoTrains(0, 1, 610), result.memory);
  expect(result.frame.screams.map((event) => event.train)).toEqual([0]);
});
