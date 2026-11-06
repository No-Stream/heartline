import { describe, expect, test } from "vitest";
import { COASTER_COLORS } from "../../coaster-colors";
import { parseTrainLayout, type TrainLayout } from "../../train-layout";
import type { Atlas } from "../scene";
import { trackPaint, trackStyle } from "./geometry";
import { buildRiderQuads, buildTrainQuads } from "./trains";

const hybrid = parseTrainLayout({ model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });
const viper = parseTrainLayout({ ...hybrid, model: "velocity_viper", trackMaterial: "steel" });
const twister = parseTrainLayout({ ...hybrid, model: "wooden_twister", trackMaterial: "wood", carsPerTrain: 12, rowsPerCar: 1 });
const viewport = { width: 400, height: 300 };
const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
const paint = trackPaint({ steel: "#000000", wood: "#000000", train: "#000000", ink: "#221133", lineWeight: 1 }, { track: "teal", supports: "cream", trains: "plum" });
/** One car pose heading east along x on level track, with no riders. */
const car = (id: number, x = 1): number[] => [id, x, 2, 3, 1, 0, 0, 0, 0, 1, 0];
const rgba = (hex: string) => [...[1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255), 1];
/** Screen width of a car heading east at rotation 0, which grows with the car's length. */
const screenLength = (layout: TrainLayout, id: number) => {
  const xs = buildTrainQuads(new Float32Array(car(id)), camera, viewport, paint, layout).flatMap((quad) => quad.vertices.map((vertex) => vertex.x));
  return Math.max(...xs) - Math.min(...xs);
};

describe("coaster trains", () => {
  test("reads the worker's train layout and refuses mismatched model metadata or counts", () => {
    expect(twister).toEqual({ model: "wooden_twister", trackMaterial: "wood", carsPerTrain: 12, rowsPerCar: 1, seatsPerRow: 2, lengthMetres: 18 });
    expect(() => parseTrainLayout({ ...hybrid, model: "monorail" } as unknown as TrainLayout)).toThrow();
    expect(() => parseTrainLayout({ ...hybrid, trackMaterial: "steel" })).toThrow();
    expect(() => parseTrainLayout({ ...hybrid, carsPerTrain: 0 })).toThrow();
    expect(() => parseTrainLayout({ ...hybrid, lengthMetres: 0 })).toThrow();
  });

  test("renders every car record when a snapshot contains multiple trains", () => {
    const quads = buildTrainQuads(new Float32Array([...car(0), ...car(1, 4)]), camera, viewport, paint, hybrid);
    expect(new Set(quads.map((quad) => quad.id))).toEqual(new Set(["train-0", "train-1"]));
    expect(quads.every((quad) => quad.kind === "train")).toBe(true);
  });

  test("paints the body in the train colour, seats in a darker shade of it and restraints in the key line", () => {
    const fills = buildTrainQuads(new Float32Array(car(1)), camera, viewport, paint, hybrid).map((quad) => quad.color.join());
    const plum = rgba(COASTER_COLORS.plum.hex);
    expect(fills).toContain(plum.join());
    expect(fills).toContain([plum[0] * 0.62, plum[1] * 0.62, plum[2] * 0.62, 1].join());
    expect(fills).toContain(rgba("#221133").join());
  });

  test("gives the front car of each train a nose and the other cars none", () => {
    const quadCount = (id: number) => buildTrainQuads(new Float32Array(car(id)), camera, viewport, paint, hybrid).length;
    expect(quadCount(0)).toBeGreaterThan(quadCount(1));
    expect(quadCount(6)).toBe(quadCount(0));
    expect(quadCount(7)).toBe(quadCount(1));
  });

  test("draws each model's cars to their own proportions", () => {
    expect(screenLength(twister, 1)).toBeLessThan(screenLength(hybrid, 1) * 0.75);
    expect(screenLength(viper, 0) - screenLength(viper, 1)).toBeGreaterThan(screenLength(hybrid, 0) - screenLength(hybrid, 1));
    const height = (layout: TrainLayout) => {
      const ys = buildTrainQuads(new Float32Array(car(1)), camera, viewport, paint, layout).flatMap((quad) => quad.vertices.map((vertex) => vertex.y));
      return Math.max(...ys) - Math.min(...ys);
    };
    expect(height(viper)).toBeGreaterThan(height(hybrid));
  });

  test("keeps every model's cars finite and depth-sorted in all four view rotations", () => {
    for (const layout of [hybrid, viper, twister]) for (const rotation of [0, 1, 2, 3] as const) {
      const quads = buildTrainQuads(new Float32Array([...car(0), ...car(1, 1.4)]), { ...camera, rotation }, viewport, paint, layout);
      expect(quads.length).toBeGreaterThan(0);
      expect(quads.every((quad) => quad.vertices.every((vertex) => Number.isFinite(vertex.x) && Number.isFinite(vertex.y) && Number.isFinite(vertex.depth)))).toBe(true);
    }
  });

  test("seats riders in rows that follow the train layout", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const atlas: Atlas = { pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: Object.fromEntries(["maya", "otis", "priya", "gus"].map((name) => [`people/${name}/se/idle/0`, frame])) };
    const full = [1, 4, 2, 3, 1, 0, 0, 0, 0, 1, 15];
    const centres = (layout: TrainLayout) => buildRiderQuads(new Float32Array(full), camera, viewport, atlas, "1", layout).map((quad) => quad.vertices.reduce((sum, vertex) => sum + vertex.x + vertex.y, 0) / 4);
    expect(centres(hybrid)).toHaveLength(4);
    expect(new Set(centres(hybrid).map((value) => value.toFixed(3))).size).toBeGreaterThan(2);
    expect(centres(twister)).toHaveLength(2);
  });

  test("uses atlas theme and rotates riders with the car", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const themeStyle = { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 };
    const atlas: Atlas = { themeStyle, pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: { "ground/grass_light_0/any/static/0": frame, "people/maya/se/idle/0": frame } };
    expect(trackStyle(atlas).steel).toBe("#ff0000");
    const poses = new Float32Array([0, 1, 2, 3, 1, 0, 0, 0, 0, 1, 1]);
    const riders = buildRiderQuads(poses, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", hybrid);
    expect(riders).toHaveLength(1);
    expect(riders[0].id).toBe("rider-0-0");
    expect(riders[0].page).toBe(0);
    expect(new Set(riders[0].vertices.map((vertex) => vertex.depth)).size).toBeGreaterThan(1);
    const actual = buildRiderQuads(poses, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", hybrid, new Map([["0-0", { id: "guest-42", sprite: "people/maya" }]]));
    expect(actual[0].id).toBe("guest-42");
    const withoutMask = new Float32Array(poses);
    withoutMask[10] = 0;
    expect(buildRiderQuads(withoutMask, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", hybrid, new Map([["0-0", { id: "guest-42", sprite: "people/maya" }]]))[0].id).toBe("guest-42");
    expect(buildRiderQuads(poses, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", hybrid, new Map())).toHaveLength(0);
  });
  test("keeps a rider visible on diagonal track while banking and inversion follow the car", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const atlas: Atlas = { pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: Object.fromEntries(["se", "sw", "nw", "ne"].map((view) => [`people/maya/${view}/idle/0`, frame])) };
    const viewport = { width: 400, height: 300 };
    const camera = { centerX: 1, centerY: 2, zoom: 1, rotation: 0 as const };
    const diagonal = Math.SQRT1_2;
    const quadFor = (up: [number, number, number]) => buildRiderQuads(new Float32Array([0, 1, 2, 3, diagonal, -diagonal, 0, ...up, 1]), camera, viewport, atlas, "1", hybrid)[0].vertices;
    const upright = quadFor([0, 0, 1]);
    const inverted = quadFor([0, 0, -1]);
    const banked = quadFor([diagonal, diagonal, 0]);
    const area = (vertices: typeof upright) => Math.abs((vertices[1].x - vertices[0].x) * (vertices[3].y - vertices[0].y) - (vertices[1].y - vertices[0].y) * (vertices[3].x - vertices[0].x));
    expect(area(upright)).toBeGreaterThan(1);
    expect(area(inverted)).toBeGreaterThan(1);
    expect(upright[0].y).toBeLessThan(upright[3].y);
    expect(inverted[0].y).toBeGreaterThan(inverted[3].y);
    expect(banked[0].y - banked[3].y).toBeGreaterThan(0);
    expect(() => quadFor([1 / Math.sqrt(3), 1 / Math.sqrt(3), 1 / Math.sqrt(3)])).not.toThrow();
    for (const rotation of [0, 1, 2, 3] as const) {
      const tangentY = rotation % 2 === 0 ? -diagonal : diagonal;
      const pose = new Float32Array([0, 1, 2, 3, diagonal, tangentY, 0, 0, 0, 1, 1]);
      const vertices = buildRiderQuads(pose, { ...camera, rotation }, viewport, atlas, "1", hybrid)[0].vertices;
      expect(area(vertices)).toBeGreaterThan(1);
    }
  });
});
