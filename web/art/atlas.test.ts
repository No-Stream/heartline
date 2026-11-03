import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

interface Placement { page: number; x: number; y: number; width: number; height: number }
interface Atlas {
  scales: number[];
  themeStyle: { steel: string; wood: string; train: string; ink: string; lineWeight: number };
  pages: Record<string, { file: string; width: number; height: number }[]>;
  sprites: Record<string, { views: Record<string, Record<string, string[]>> }>;
  frames: Record<string, { source: string; sourceHash: string; width: number; height: number; properties?: Record<string, string>; placements: Record<string, Placement> }>;
}

const artDirectory = dirname(fileURLToPath(import.meta.url));
const drawingsDirectory = join(artDirectory, "drawings");
const atlas = JSON.parse(readFileSync(join(artDirectory, "atlas", "atlas.json"), "utf8")) as Atlas;

function listSvgFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return listSvgFiles(path);
    return entry.name.endsWith(".svg") ? [relative(drawingsDirectory, path).split(sep).join("/")] : [];
  });
}

function spritesIn(category: string): [string, Atlas["sprites"][string]][] {
  return Object.entries(atlas.sprites).filter(([name]) => name.startsWith(`${category}/`));
}

describe("sprite atlas metadata", () => {
  test("exports coaster colors and line weight once at atlas level", () => {
    expect(atlas.themeStyle).toEqual({ steel: expect.any(String), wood: expect.any(String), train: expect.any(String), ink: expect.any(String), lineWeight: expect.any(Number) });
    expect(Object.values(atlas.frames).some((frame) => Object.keys(frame.properties ?? {}).some((key) => key.startsWith("coaster-")))).toBe(false);
  });

  test("covers every source drawing and nothing else, built from the current files", () => {
    const framesBySource = new Map(Object.values(atlas.frames).map((frame) => [frame.source, frame]));
    expect([...framesBySource.keys()].sort()).toEqual(listSvgFiles(drawingsDirectory).sort());
    for (const [source, frame] of framesBySource) {
      const currentHash = createHash("sha256").update(readFileSync(join(drawingsDirectory, source), "utf8")).digest("hex");
      expect(frame.sourceHash, `${source} changed; run make art`).toBe(currentHash);
    }
  });

  test("every frame has a placement inside an existing page at every scale", () => {
    expect(atlas.scales).toEqual(expect.arrayContaining([1, 2]));
    for (const scale of atlas.scales) {
      for (const page of atlas.pages[scale]) {
        const png = readFileSync(join(artDirectory, "atlas", page.file));
        expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([page.width, page.height]);
        expect(Math.max(page.width, page.height)).toBeLessThanOrEqual(4096);
      }
      for (const [key, frame] of Object.entries(atlas.frames)) {
        const placement = frame.placements[scale];
        const page = atlas.pages[scale][placement.page];
        expect(page, key).toBeDefined();
        expect(placement.x + placement.width, key).toBeLessThanOrEqual(page.width);
        expect(placement.y + placement.height, key).toBeLessThanOrEqual(page.height);
        expect([placement.width, placement.height], key).toEqual([Math.ceil(frame.width * scale), Math.ceil(frame.height * scale)]);
      }
    }
  });

  test("the sprite index points at existing frames with no gaps", () => {
    const indexedKeys = Object.values(atlas.sprites).flatMap(({ views }) => Object.values(views).flatMap((animations) => Object.values(animations).flat()));
    expect(new Set(indexedKeys)).toEqual(new Set(Object.keys(atlas.frames)));
    for (const [sprite, { views }] of Object.entries(atlas.sprites)) {
      for (const [view, animations] of Object.entries(views)) {
        for (const [animation, frames] of Object.entries(animations)) {
          expect(frames).toEqual(frames.map((_, frame) => `${sprite}/${view}/${animation}/${frame}`));
        }
      }
    }
  });

  test("content meets the art proof's minimums", () => {
    const people = spritesIn("people");
    expect(people.length).toBeGreaterThanOrEqual(8);
    for (const [name, { views }] of people) {
      expect(Object.keys(views).sort(), name).toEqual(["ne", "nw", "se", "sw"]);
      for (const animations of Object.values(views)) {
        expect(animations.walk.length, name).toBeGreaterThanOrEqual(6);
        expect(animations.idle.length, name).toBeGreaterThanOrEqual(1);
      }
    }
    const buildings = spritesIn("buildings");
    expect(buildings.map(([name]) => name)).toEqual(expect.arrayContaining(["buildings/burger_stall", "buildings/restroom", "buildings/ticket_booth", "buildings/souvenir_shop"]));
    for (const [name, { views }] of buildings) {
      expect(Object.keys(views).sort(), name).toEqual(["rot0", "rot180", "rot270", "rot90"]);
    }
    expect(spritesIn("scenery").length).toBeGreaterThanOrEqual(8);
    expect(existsSync(drawingsDirectory)).toBe(true);
  });
});
