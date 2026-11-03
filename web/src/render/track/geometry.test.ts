import { describe, expect, test } from "vitest";
import { buildRiderQuads, buildTrackQuads, interpolateTrainPoses, trackStyle } from "./geometry";
import type { Atlas } from "../scene";

const rail = [1, 1, 2, 1, 0, 1, 0, 0, 0, 1, 3, 2, 2, 0, 1, 0, 0, 0, 1];
const tie = [1, 1, 1.8, 1, 1, 2.2, 1, 0, 0, 1, 1, 0, 0];

describe("coaster geometry", () => {
  test("renders signed steel geometry as tubes and restores the original picking IDs", () => {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const style = { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 };
    const steelRail = new Float32Array([-2, ...rail.slice(1)]);
    const steelTie = new Float32Array([-3, ...tie.slice(1)]);
    const supports = new Float32Array([
      -4, 2, 2, 0, 2, 2, 3,
      5, 3, 2, 0, 3, 2, 3,
    ]);

    const quads = buildTrackQuads(steelRail, steelTie, supports, new Float32Array(), camera, { width: 400, height: 300 }, style);
    const railQuads = quads.filter((quad) => quad.id === "track-1");
    const tieQuads = quads.filter((quad) => quad.id === "track-2");
    const steelSupportQuads = quads.filter((quad) => quad.id === "support-3");
    const woodSupportQuads = quads.filter((quad) => quad.id === "support-5");

    expect(railQuads).toHaveLength(8);
    expect(railQuads.every((quad) => quad.color[0] > 0 && quad.color[0] <= 1 && quad.color[1] === 0 && quad.color[2] === 0)).toBe(true);
    expect(tieQuads).toHaveLength(8);
    expect(tieQuads.every((quad) => quad.color[0] > 0 && quad.color[0] <= 1 && quad.color[1] === 0 && quad.color[2] === 0)).toBe(true);
    expect(steelSupportQuads).toHaveLength(8);
    expect(steelSupportQuads.every((quad) => quad.color[0] > 0 && quad.color[0] <= 1 && quad.color[1] === 0 && quad.color[2] === 0)).toBe(true);
    expect(woodSupportQuads[0].color).toEqual([170 / 255, 102 / 255, 0, 1]);
    expect(quads.some((quad) => quad.id.includes("--"))).toBe(false);
  });

  test("adds a footer to each vertical support column, not its horizontal brace", () => {
    const style = { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 };
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const supports = new Float32Array([1, 2, 2, 0, 2, 2, 3, 1, 2, 2, 3, 3, 2, 3]);
    const quads = buildTrackQuads(new Float32Array(), new Float32Array(), supports, new Float32Array(), camera, { width: 400, height: 300 }, style);
    expect(quads.filter((quad) => quad.id === "footer-1")).toHaveLength(30);
    expect(quads.filter((quad) => quad.id === "support-1")).toHaveLength(60);
  });
  test("generates geometric rails with depth and picking identity in every camera rotation", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(tie), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation }, { width: 400, height: 300 }, { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 });
      expect(quads.length).toBeGreaterThan(0);
      expect(quads.every((quad) => quad.id === "track-1")).toBe(true);
      expect(quads.some((quad) => new Set(quad.vertices.map((vertex) => vertex.depth)).size > 1)).toBe(true);
    }
  });
  test("does not draw inked caps at joins between rail segments", () => {
    const second = [1, 3, 2, 2, 0, 1, 0, 0, 0, 1, 5, 2, 3, 0, 1, 0, 0, 0, 1];
    const quads = buildTrackQuads(new Float32Array([...rail, ...second]), new Float32Array(), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 });
    // Four faces with one fill and two longitudinal ink edges per segment.
    expect(quads).toHaveLength(24);
  });
  test("partitions each rail face into non-overlapping fill and ink", () => {
    const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 });
    for (let face = 0; face < quads.length; face += 3) {
      const [fill, firstEdge, secondEdge] = quads.slice(face, face + 3);
      expect(fill.vertices[0]).toEqual(firstEdge.vertices[3]);
      expect(fill.vertices[1]).toEqual(firstEdge.vertices[2]);
      expect(fill.vertices[2]).toEqual(secondEdge.vertices[3]);
      expect(fill.vertices[3]).toEqual(secondEdge.vertices[2]);
    }
  });
  test("keeps rail ink visible at normal viewing scale", () => {
    const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1.3, rotation: 0 }, { width: 400, height: 300 }, { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 });
    const edge = quads[1].vertices;
    expect(Math.hypot(edge[0].x - edge[3].x, edge[0].y - edge[3].y)).toBeGreaterThan(0.5);
  });
  test("partitions a support face into non-overlapping fill and four ink borders", () => {
    const supports = new Float32Array([1, 2, 2, 0, 2, 2, 3]);
    const quads = buildTrackQuads(new Float32Array(), new Float32Array(), supports, new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 });
    const [fill, ...edges] = quads.slice(0, 5);
    for (let edge = 0; edge < 4; edge++) {
      expect(edges[edge].vertices[3]).toEqual(fill.vertices[edge]);
      expect(edges[edge].vertices[2]).toEqual(fill.vertices[(edge + 1) % 4]);
    }
  });
  test("keeps the support ink width fixed as the column height increases", () => {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const style = { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 };
    const faceCornerInset = (height: number) => {
      const supports = new Float32Array([1, 2, 2, 0, 2, 2, height]);
      const quads = buildTrackQuads(new Float32Array(), new Float32Array(), supports, new Float32Array(), camera, { width: 400, height: 300 }, style);
      const fill = quads[10].vertices[0];
      const corner = quads[11].vertices[0];
      return { x: fill.x - corner.x, y: fill.y - corner.y, depth: fill.depth - corner.depth };
    };
    const short = faceCornerInset(3);
    const tall = faceCornerInset(30);
    expect(tall.x).toBeCloseTo(short.x, 8);
    expect(tall.y).toBeCloseTo(short.y, 8);
    expect(tall.depth).toBeCloseTo(short.depth, 8);
  });
  test("interpolates poses and keeps rider occupancy from current tick", () => {
    const previous = new Float32Array([0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0]);
    const current = new Float32Array([0, 2, 4, 6, 0, 1, 0, 0, 0, 1, 3]);
    expect([...interpolateTrainPoses(previous, current, 0.5)]).toEqual([0, 1, 2, 3, 0, 1, 0, 0, 0, 1, 3]);
  });
  test("uses atlas theme and rotates riders with the car", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const themeStyle = { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 };
    const atlas: Atlas = { themeStyle, pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: { "ground/grass_light_0/any/static/0": frame, "people/maya/se/idle/0": frame } };
    expect(trackStyle(atlas).steel).toBe("#ff0000");
    const poses = new Float32Array([0, 1, 2, 3, 1, 0, 0, 0, 0, 1, 1]);
    const riders = buildRiderQuads(poses, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1");
    expect(riders).toHaveLength(1);
    expect(riders[0].id).toBe("rider-0-0");
    expect(riders[0].page).toBe(0);
    expect(new Set(riders[0].vertices.map((vertex) => vertex.depth)).size).toBeGreaterThan(1);
    const actual = buildRiderQuads(poses, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", new Map([["0-0", { id: "guest-42", sprite: "people/maya" }]]));
    expect(actual[0].id).toBe("guest-42");
    const withoutMask = new Float32Array(poses);
    withoutMask[10] = 0;
    expect(buildRiderQuads(withoutMask, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", new Map([["0-0", { id: "guest-42", sprite: "people/maya" }]]))[0].id).toBe("guest-42");
    expect(buildRiderQuads(poses, { centerX: 1, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, atlas, "1", new Map())).toHaveLength(0);
  });
  test("keeps a rider visible on diagonal track while banking and inversion follow the car", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const atlas: Atlas = { pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: Object.fromEntries(["se", "sw", "nw", "ne"].map((view) => [`people/maya/${view}/idle/0`, frame])) };
    const viewport = { width: 400, height: 300 };
    const camera = { centerX: 1, centerY: 2, zoom: 1, rotation: 0 as const };
    const diagonal = Math.SQRT1_2;
    const quadFor = (up: [number, number, number]) => buildRiderQuads(new Float32Array([0, 1, 2, 3, diagonal, -diagonal, 0, ...up, 1]), camera, viewport, atlas, "1")[0].vertices;
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
      const vertices = buildRiderQuads(pose, { ...camera, rotation }, viewport, atlas, "1")[0].vertices;
      expect(area(vertices)).toBeGreaterThan(1);
    }
  });
});
