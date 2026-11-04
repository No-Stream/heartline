import { describe, expect, test } from "vitest";
import { buildTrackQuads, interpolateTrainPoses, trackPaint, trackStyle } from "./geometry";
import { COASTER_COLORS, RUNNING_RAIL_HEX } from "../../coaster-colors";
import type { Atlas } from "../scene";

const rail = [1, 1, 2, 1, 0, 1, 0, 0, 0, 1, 3, 2, 2, 0, 1, 0, 0, 0, 1];
const tie = [1, 1, 1.8, 1, 1, 2.2, 1, 0, 0, 1, 1, 0, 0];

describe("coaster geometry", () => {
  test("renders wooden rails in the track paint with dark steel running strips", () => {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const style = { track: "#aa6600", supports: "#ff0000", trains: "#00ff00", ink: "#221133", lineWeight: 1 };
    const woodenRail = new Float32Array([1_000_001, ...rail.slice(1)]);
    const quads = buildTrackQuads(woodenRail, new Float32Array(), new Float32Array(), camera, { width: 400, height: 300 }, style);
    const runningRail = [1, 3, 5].map((offset) => parseInt(RUNNING_RAIL_HEX.slice(offset, offset + 2), 16) / 255);
    expect(quads.every((quad) => quad.id === "track-1")).toBe(true);
    expect(quads.some((quad) => quad.color[0] === 170 / 255 && quad.color[1] === 102 / 255)).toBe(true);
    expect(quads.some((quad) => quad.color.slice(0, 3).every((channel, index) => channel === runningRail[index]))).toBe(true);
    expect(quads.some((quad) => quad.color[0] === 1 && quad.color[1] === 0)).toBe(false);
  });

  test("paints track and ties and supports each in the coaster's own colours", () => {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const paint = trackPaint({ steel: "#000000", wood: "#000000", train: "#000000", ink: "#221133", lineWeight: 1 }, { track: "teal", supports: "cream", trains: "plum" });
    const supports = new Float32Array([4, 2, 2, 0, 2, 2, 3]);
    const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(tie), supports, camera, { width: 400, height: 300 }, paint);
    const fills = (id: string) => new Set(quads.filter((quad) => quad.id === id && quad.color.join() !== [34 / 255, 17 / 255, 51 / 255, 1].join()).map((quad) => quad.color.join()));
    const rgba = (hex: string) => [...[1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255), 1].join();
    expect(fills("track-1")).toEqual(new Set([rgba(COASTER_COLORS.teal.hex)]));
    expect(fills("support-4")).toEqual(new Set([rgba(COASTER_COLORS.cream.hex)]));
  });
  test("renders signed steel geometry as tubes and restores the original picking IDs", () => {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const style = { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 };
    const steelRail = new Float32Array([-2, ...rail.slice(1)]);
    const steelTie = new Float32Array([-3, ...tie.slice(1)]);
    const supports = new Float32Array([
      -4, 2, 2, 0, 2, 2, 3,
      5, 3, 2, 0, 3, 2, 3,
    ]);

    const quads = buildTrackQuads(steelRail, steelTie, supports, camera, { width: 400, height: 300 }, style);
    const railQuads = quads.filter((quad) => quad.id === "track-1");
    const tieQuads = quads.filter((quad) => quad.id === "track-2");
    const steelSupportQuads = quads.filter((quad) => quad.id === "support-3");
    const woodSupportQuads = quads.filter((quad) => quad.id === "support-5");

    // An eight-sided tube shows the viewer the four sides that face it.
    expect(railQuads).toHaveLength(4);
    expect(railQuads.every((quad) => quad.color[0] > 0 && quad.color[0] <= 1 && quad.color[1] === 0 && quad.color[2] === 0)).toBe(true);
    expect(tieQuads).toHaveLength(4);
    expect(tieQuads.every((quad) => quad.color[0] > 0 && quad.color[0] <= 1 && quad.color[1] === 0 && quad.color[2] === 0)).toBe(true);
    expect(steelSupportQuads).toHaveLength(4);
    for (const quad of steelSupportQuads) {
      expect(quad.color[1] / quad.color[0]).toBeCloseTo(102 / 170, 6);
      expect(quad.color[2]).toBe(0);
    }
    expect(woodSupportQuads[0].color).toEqual([170 / 255, 102 / 255, 0, 1]);
    expect(quads.some((quad) => quad.id.includes("--"))).toBe(false);
  });

  test("adds a footer to each vertical support column, not its horizontal brace", () => {
    const style = { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 };
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const supports = new Float32Array([1, 2, 2, 0, 2, 2, 3, 1, 2, 2, 3, 3, 2, 3]);
    const quads = buildTrackQuads(new Float32Array(), new Float32Array(), supports, camera, { width: 400, height: 300 }, style);
    // Each inked box shows the viewer three faces, each a fill and four ink borders; the brace has three plain faces.
    expect(quads.filter((quad) => quad.id === "footer-1")).toHaveLength(15);
    expect(quads.filter((quad) => quad.id === "support-1")).toHaveLength(18);
  });
  test("draws only the support faces that face the viewer, top included, in every rotation", () => {
    const style = { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 };
    const column = new Float32Array([1, 2, 2, 0, 2, 2, 3]);
    for (const rotation of [0, 1, 2, 3] as const) {
      const camera = { centerX: 2, centerY: 2, zoom: 1, rotation };
      const quads = buildTrackQuads(new Float32Array(), new Float32Array(), column, camera, { width: 400, height: 300 }, style).filter((quad) => quad.id === "support-1");
      const fills = quads.filter((_, index) => index % 5 === 0);
      expect(fills).toHaveLength(3);
      const topY = Math.min(...quads.flatMap((quad) => quad.vertices.map((vertex) => vertex.y)));
      expect(fills.some((fill) => fill.vertices.every((vertex) => vertex.y < topY + 10))).toBe(true);
    }
  });
  test("draws support columns thicker than their diagonal braces", () => {
    const style = { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 };
    const camera = { centerX: 2, centerY: 2, zoom: 2, rotation: 0 as const };
    const widthOf = (segment: number[]) => {
      const quads = buildTrackQuads(new Float32Array(), new Float32Array(), new Float32Array([1, ...segment]), camera, { width: 400, height: 300 }, style).filter((quad) => quad.id === "support-1");
      const xs = quads.flatMap((quad) => quad.vertices.map((vertex) => vertex.x));
      return Math.max(...xs) - Math.min(...xs);
    };
    // The brace rises along x + y, so like the column its axis stays on one screen column and only its thickness shows.
    expect(widthOf([2, 2, 0, 2, 2, 2]) / widthOf([2, 2, 0, 2.5, 2.5, 1])).toBeGreaterThan(1.5);
  });
  test("generates geometric rails with depth and picking identity in every camera rotation", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(tie), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation }, { width: 400, height: 300 }, { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 });
      expect(quads.length).toBeGreaterThan(0);
      expect(quads.every((quad) => quad.id === "track-1")).toBe(true);
      expect(quads.some((quad) => new Set(quad.vertices.map((vertex) => vertex.depth)).size > 1)).toBe(true);
    }
  });
  test("does not draw inked caps at joins between rail segments", () => {
    const second = [1, 3, 2, 2, 0, 1, 0, 0, 0, 1, 5, 2, 3, 0, 1, 0, 0, 0, 1];
    const quads = buildTrackQuads(new Float32Array([...rail, ...second]), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 });
    // Four faces with one fill and two longitudinal ink edges per segment.
    expect(quads).toHaveLength(24);
  });
  test("partitions each rail face into non-overlapping fill and ink", () => {
    const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 });
    for (let face = 0; face < quads.length; face += 3) {
      const [fill, firstEdge, secondEdge] = quads.slice(face, face + 3);
      expect(fill.vertices[0]).toEqual(firstEdge.vertices[3]);
      expect(fill.vertices[1]).toEqual(firstEdge.vertices[2]);
      expect(fill.vertices[2]).toEqual(secondEdge.vertices[3]);
      expect(fill.vertices[3]).toEqual(secondEdge.vertices[2]);
    }
  });
  test("keeps rail ink visible at normal viewing scale", () => {
    const quads = buildTrackQuads(new Float32Array(rail), new Float32Array(), new Float32Array(), { centerX: 2, centerY: 2, zoom: 1.3, rotation: 0 }, { width: 400, height: 300 }, { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 });
    const edge = quads[1].vertices;
    expect(Math.hypot(edge[0].x - edge[3].x, edge[0].y - edge[3].y)).toBeGreaterThan(0.5);
  });
  test("partitions a support face into non-overlapping fill and four ink borders", () => {
    const supports = new Float32Array([1, 2, 2, 0, 2, 2, 3]);
    const quads = buildTrackQuads(new Float32Array(), new Float32Array(), supports, { centerX: 2, centerY: 2, zoom: 1, rotation: 0 }, { width: 400, height: 300 }, { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 });
    const [fill, ...edges] = quads.slice(0, 5);
    for (let edge = 0; edge < 4; edge++) {
      expect(edges[edge].vertices[3]).toEqual(fill.vertices[edge]);
      expect(edges[edge].vertices[2]).toEqual(fill.vertices[(edge + 1) % 4]);
    }
  });
  test("keeps the support ink width fixed as the column height increases", () => {
    const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
    const style = { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 };
    const faceCornerInset = (height: number) => {
      const supports = new Float32Array([1, 2, 2, 0, 2, 2, height]);
      const quads = buildTrackQuads(new Float32Array(), new Float32Array(), supports, camera, { width: 400, height: 300 }, style);
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
});
