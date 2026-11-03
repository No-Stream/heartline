import { describe, expect, test } from "vitest";
import { appendGeometry, buildScene, pickScene, type Atlas, type GeometryQuad, type WorldView } from "./scene";
import type { Camera } from "../camera";
import { buildPathGhostQuads } from "./path-ghost";

const frame = { width: 72, height: 40, anchor: { x: 36, y: 20 }, placements: { "1": { page: 0, x: 0, y: 0, width: 72, height: 40 } } };
const atlas: Atlas = {
  pages: { "1": [{ file: "page.png", width: 72, height: 40 }] },
  frames: {
    "ground/grass_light_0/any/static/0": frame,
    "ground/path_0000/any/static/0": frame,
    "scenery/tree/any/static/0": frame,
    ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
      [`ground/cliff_${side}_${variant}/any/static/0`, frame],
      [`ground/cliff_step_${side}_${variant}/any/static/0`, frame],
    ]))),
    "ground/water/any/static/0": frame,
    ...Object.fromEntries([0, 1, 2, 3].map((waterFrame) => [`ground/water/any/loop/${waterFrame}`, frame])),
  },
};
const camera: Camera = { centerX: 1, centerY: 1, zoom: 1, rotation: 0 };
const viewport = { width: 400, height: 300 };
const world: WorldView = { dimensions: new Uint16Array([2, 2]), surfaces: new Int32Array([0,0,0,0,0,0, 0,0,0,0,0,0, 0,0,0,0,0,0, 0,0,0,0,0,0]), paths: new Int32Array() };

function atlasWithDistinctTerrainFrames(): { atlas: Atlas; pagesByFrame: Map<string, number> } {
  const frames = { ...atlas.frames };
  const pages = [...atlas.pages["1"]];
  const pagesByFrame = new Map<string, number>();
  const cliffFrames = ["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
    [`ground/cliff_${side}_${variant}/any/static/0`, 40, 76, side === "left" ? 2 : 38, 2],
    [`ground/cliff_step_${side}_${variant}/any/static/0`, 44, 36, side === "left" ? 4 : 40, 4],
  ] as const));
  const waterFrames = [0, 1, 2, 3].map((waterFrame) => [`ground/water/any/loop/${waterFrame}`, 72, 40, 36, 20] as const);

  for (const [key, width, height, anchorX, anchorY] of [...cliffFrames, ...waterFrames]) {
    const page = pages.length;
    pagesByFrame.set(key, page);
    pages.push({ file: `${page}.png`, width, height });
    frames[key] = {
      width,
      height,
      anchor: { x: anchorX, y: anchorY },
      placements: { "1": { page, x: 0, y: 0, width, height } },
    };
  }

  return { atlas: { pages: { "1": pages }, frames }, pagesByFrame };
}

describe("world scene", () => {
  test("scales sprites around their anchor while preserving their picking identity", () => {
    const sprite = { id: "facility-1", sprite: "scenery/tree", x: 1, y: 1, height: 0 };
    const full = buildScene({ ...world, sprites: [sprite] }, camera, viewport, atlas).triangles.filter((triangle) => triangle.id === sprite.id);
    const half = buildScene({ ...world, sprites: [{ ...sprite, scale: 0.5 }] }, camera, viewport, atlas).triangles.filter((triangle) => triangle.id === sprite.id);
    expect(half[0].vertices[1].x - half[0].vertices[0].x).toBe((full[0].vertices[1].x - full[0].vertices[0].x) / 2);
    const [a, b, c] = half[0].vertices;
    expect(pickScene({ triangles: half }, (a.x + b.x + c.x) / 3, (a.y + b.y + c.y) / 3)).toMatchObject({ id: sprite.id });
  });
  test("extra geometry shares depth and picking with terrain", () => {
    const scene = buildScene(world, camera, viewport, atlas);
    const terrain = scene.triangles.find((triangle) => triangle.kind === "terrain")!;
    const [a, b, c] = terrain.vertices;
    const x = (a.x + b.x + c.x) / 3;
    const y = (a.y + b.y + c.y) / 3;
    const quad = (depth: number): GeometryQuad => ({
      vertices: [[x-2,y-2], [x+2,y-2], [x+2,y+2], [x-2,y+2]].map(([px, py]) => ({ x: px, y: py, depth, height: 7, u: 0, v: 0 })) as GeometryQuad["vertices"],
      kind: "fixture", id: "one", tileX: 0, tileY: 0, color: [1, 0, 0, 1],
    });
    appendGeometry(scene, [quad(-100)]);
    expect(pickScene(scene, x, y)?.kind).toBe("terrain");
    appendGeometry(scene, [quad(100)]);
    expect(pickScene(scene, x, y)).toEqual({ kind: "fixture", id: "one", x: 0, y: 0, height: 7 });
    const outside = quad(100);
    outside.vertices.forEach((vertex) => { vertex.x += 1000; });
    const before = scene.triangles.length;
    appendGeometry(scene, [outside], viewport);
    expect(scene.triangles).toHaveLength(before);
  });
  test("resolves a rasterized subpixel edge to the selected geometry", () => {
    const vertices = [[10.516, 0], [10.696, 0], [10.696, 20], [10.516, 20]].map(([x, y]) => ({ x, y, depth: 1, height: 2, u: 0, v: 0 })) as GeometryQuad["vertices"];
    const scene = { triangles: [] };
    appendGeometry(scene, [{ vertices, kind: "fixture", id: "thin", tileX: 0, tileY: 0, color: [1, 1, 1, 1] }]);
    expect(pickScene(scene, 10.5, 10)).toBeNull();
    expect(pickScene(scene, 10.5, 10, 0.125)).toMatchObject({ kind: "fixture", id: "thin" });
  });
  test("projects and picks each tile in every rotation", () => {
    for (const rotation of [0, 1, 2, 3] as const) {
      const scene = buildScene(world, { ...camera, rotation }, viewport, atlas);
      const terrain = scene.triangles.filter((item) => item.kind === "terrain");
      expect(terrain).toHaveLength(8);
      for (const triangle of terrain) {
        const x = triangle.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 3;
        const y = triangle.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 3;
        expect(pickScene(scene, x, y)).toEqual({ kind: "terrain", x: triangle.tileX, y: triangle.tileY, height: 0 });
      }
    }
  });
  test("draws slopes, cliffs, water and paths and picks the visible surface", () => {
    const surfaces = new Int32Array(world.surfaces);
    surfaces.set([4, 4, 8, 8, 1, 9], 0);
    const scene = buildScene({ ...world, surfaces, paths: new Int32Array([1, 1, 0, 0]) }, camera, viewport, atlas);
    expect(scene.triangles.some((triangle) => triangle.kind === "cliff")).toBe(true);
    expect(scene.triangles.some((triangle) => triangle.kind === "water")).toBe(true);
    expect(scene.triangles.some((triangle) => triangle.kind === "path")).toBe(true);
    const water = scene.triangles.find((triangle) => triangle.kind === "water")!;
    const x = water.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 3;
    const y = water.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 3;
    expect(pickScene(scene, x, y)?.kind).toBe("water");
  });
  test("keeps terrain texture coordinates when a cliff shares its corners", () => {
    const surfaces = new Int32Array(world.surfaces);
    surfaces.set([4, 4, 4, 4, 0, 0]);
    const scene = buildScene({ ...world, surfaces }, camera, viewport, atlas);
    const terrain = scene.triangles.find((triangle) => triangle.kind === "terrain" && triangle.tileX === 0 && triangle.tileY === 0)!;
    expect(scene.triangles.some((triangle) => triangle.kind === "cliff" && triangle.tileX === 0 && triangle.tileY === 0)).toBe(true);
    expect(terrain.vertices.map(({ u, v }) => [u, v])).toEqual([[0.5, 0.1], [0.944444, 0.5], [0.5, 0.9]]);
  });
  test("uses the matching cliff face and maps its artwork in every rotation", () => {
    const cliffFrame = (anchorX: number, page: number) => ({ width: 40, height: 76, anchor: { x: anchorX, y: 2 }, placements: { "1": { page, x: 0, y: 0, width: 40, height: 76 } } });
    const cliffAtlas: Atlas = { pages: { "1": [atlas.pages["1"][0], { file: "left.png", width: 40, height: 76 }, { file: "right.png", width: 40, height: 76 }] }, frames: { ...atlas.frames, "ground/cliff_left_0/any/static/0": cliffFrame(2, 1), "ground/cliff_right_0/any/static/0": cliffFrame(38, 2) } };
    const raised: WorldView = { dimensions: new Uint16Array([1, 1]), surfaces: new Int32Array([8, 8, 8, 8, 0, 0]), paths: new Int32Array() };
    for (const rotation of [0, 1, 2, 3] as const) {
      const cliffs = buildScene(raised, { ...camera, rotation }, viewport, cliffAtlas).triangles.filter((triangle) => triangle.kind === "cliff");
      expect(new Set(cliffs.map((triangle) => triangle.page))).toEqual(new Set([1, 2]));
      for (const triangle of cliffs) for (const vertex of triangle.vertices) {
        const pixelX = vertex.u * 40; const pixelY = vertex.v * 76;
        expect([2, 34, 6, 38]).toContain(pixelX);
        expect([2, 18, 46, 62]).toContain(pixelY);
      }
    }
  });
  test("chooses repeatable cliff variants by tile position", () => {
    const { atlas: variantAtlas, pagesByFrame } = atlasWithDistinctTerrainFrames();
    const surfaces = new Int32Array(7 * 3 * 6);
    for (const x of [1, 3, 5]) surfaces.set([2, 2, 2, 2, 0, 0], (7 + x) * 6);
    const raisedTiles: WorldView = { dimensions: new Uint16Array([7, 3]), surfaces, paths: new Int32Array() };
    const selectedVariant = (rotation: Camera["rotation"], tileX: number): number => {
      const cliffs = buildScene(raisedTiles, { ...camera, centerX: 3, centerY: 1, rotation }, viewport, variantAtlas).triangles
        .filter((triangle) => triangle.kind === "cliff" && triangle.tileX === tileX && triangle.tileY === 1);
      const variantKeys = new Set(cliffs.map((triangle) => {
        const frameKey = [...pagesByFrame].find(([, page]) => page === triangle.page)?.[0];
        expect(frameKey).toMatch(/^ground\/cliff_step_(left|right)_[0-2]\/any\/static\/0$/);
        return Number(frameKey!.match(/_(\d+)\/any\//)?.[1]);
      }));
      expect(variantKeys.size).toBe(1);
      return [...variantKeys][0];
    };

    const variantsByTile = [1, 3, 5].map((tileX) => selectedVariant(0, tileX));
    expect(selectedVariant(0, 1)).toBe(variantsByTile[0]);
    expect(selectedVariant(1, 1)).toBe(variantsByTile[0]);
    expect(selectedVariant(2, 1)).toBe(variantsByTile[0]);
    expect(selectedVariant(3, 1)).toBe(variantsByTile[0]);
    expect(new Set(variantsByTile).size).toBeGreaterThan(1);
  });
  test("stacks one 8px cliff sprite for each interior height step", () => {
    const { atlas: cliffAtlas, pagesByFrame } = atlasWithDistinctTerrainFrames();
    const surfaces = new Int32Array(3 * 3 * 6);
    surfaces.set([3, 3, 3, 3, 0, 0], (3 + 1) * 6);
    const scene = buildScene({ dimensions: new Uint16Array([3, 3]), surfaces, paths: new Int32Array() }, { ...camera, centerX: 1, centerY: 1 }, viewport, cliffAtlas);
    const cliffs = scene.triangles.filter((triangle) => triangle.kind === "cliff" && triangle.tileX === 1 && triangle.tileY === 1);

    expect(cliffs).toHaveLength(12);
    expect(new Set(cliffs.map((triangle) => triangle.page))).toEqual(new Set([
      pagesByFrame.get("ground/cliff_step_left_0/any/static/0"),
      pagesByFrame.get("ground/cliff_step_right_0/any/static/0"),
    ]));
    expect(new Set(cliffs.map((triangle) => Math.min(...triangle.vertices.map((vertex) => vertex.height))))).toEqual(new Set([0, 1, 2]));
    for (const triangle of cliffs) {
      const heights = triangle.vertices.map((vertex) => vertex.height);
      expect(Math.max(...heights) - Math.min(...heights)).toBe(1);
    }
  });
  test("draws deep map-edge faces below flat ground", () => {
    const { atlas: cliffAtlas, pagesByFrame } = atlasWithDistinctTerrainFrames();
    const cliffs = buildScene(world, camera, viewport, cliffAtlas).triangles.filter((triangle) => triangle.kind === "cliff");
    const deepCliffPages = new Set([...pagesByFrame].filter(([key]) => key.includes("/cliff_") && !key.includes("cliff_step_")).map(([, page]) => page));

    expect(cliffs.length).toBeGreaterThan(0);
    expect(cliffs.every((triangle) => deepCliffPages.has(triangle.page))).toBe(true);
    expect(cliffs.every((triangle) => triangle.vertices.some((vertex) => vertex.height < 0))).toBe(true);
  });
  test("selects each water loop frame", () => {
    const { atlas: waterAtlas, pagesByFrame } = atlasWithDistinctTerrainFrames();
    const waterWorld: WorldView = {
      dimensions: new Uint16Array([1, 1]),
      surfaces: new Int32Array([0, 0, 0, 0, 1, 2]),
      paths: new Int32Array(),
    };

    for (const waterFrame of [0, 1, 2, 3]) {
      const water = buildScene(waterWorld, camera, viewport, waterAtlas, "1", waterFrame).triangles.filter((triangle) => triangle.kind === "water");
      expect(water).toHaveLength(2);
      expect(new Set(water.map((triangle) => triangle.page))).toEqual(new Set([pagesByFrame.get(`ground/water/any/loop/${waterFrame}`)]));
    }
  });
  test("retains stacked paths and interpolates a sloped surface height", () => {
    const surfaces = new Int32Array(world.surfaces);
    surfaces.set([0, 1, 1, 0, 0, 0]);
    const scene = buildScene({ ...world, surfaces, paths: new Int32Array([1, 1, 0, 0, 1, 1, 8, 0]) }, camera, viewport, atlas);
    expect(scene.triangles.filter((triangle) => triangle.kind === "path")).toHaveLength(4);
    const triangle = scene.triangles.find((item) => item.kind === "terrain" && item.tileX === 0 && item.tileY === 0)!;
    const x = triangle.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 3;
    const y = triangle.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 3;
    expect(pickScene({ triangles: [triangle] }, x, y)?.height).toBeCloseTo(2 / 3);
  });
  test("rotates path connections and matches slopes at shared edges", () => {
    const masks = ["1000", "0100", "0010", "0001"];
    const pages = Array.from({ length: 5 }, () => ({ file: "page.png", width: 72, height: 40 }));
    const frames = { ...atlas.frames };
    masks.forEach((mask, index) => { frames[`ground/path_${mask}/any/static/0`] = { ...frame, placements: { "1": { ...frame.placements["1"], page: index + 1 } } }; });
    const connectedAtlas = { ...atlas, pages: { "1": pages }, frames };
    for (const rotation of [0, 1, 2, 3] as const) {
      const scene = buildScene({ ...world, paths: new Int32Array([0, 0, 0, 0, 1, 0, 0, 0]) }, { ...camera, rotation }, viewport, connectedAtlas);
      expect(scene.triangles.find((item) => item.kind === "path" && item.tileX === 0 && item.tileY === 0)?.page).toBe(rotation + 1);
    }
  });
  test("draws queue railings only on unconnected edges and distinguishes path styles", () => {
    const connectedAtlas: Atlas = { ...atlas, frames: { ...atlas.frames, ...Object.fromEntries(["1000", "0010"].map((bits) => [`ground/path_${bits}/any/static/0`, frame])) } };
    const scene = buildScene({ ...world, paths: new Int32Array([0, 0, 0, 0, 1, 0, 0, 0]), pathMetadata: new Int32Array([1, 1, 0, 0]) }, camera, viewport, connectedAtlas);
    expect(scene.triangles.filter((triangle) => triangle.kind === "path-railing")).toHaveLength(6);
    const queue = scene.triangles.find((triangle) => triangle.kind === "path" && triangle.tileX === 0)!;
    const normal = scene.triangles.find((triangle) => triangle.kind === "path" && triangle.tileX === 1)!;
    expect(queue.color).not.toEqual(normal.color);
    expect(() => buildScene({ ...world, paths: new Int32Array([0, 0, 0, 0]), pathMetadata: new Int32Array([1]) }, camera, viewport, atlas)).toThrow();
  });
  test("queue railing geometry picks as its underlying path", () => {
    const scene = buildScene({ ...world, paths: new Int32Array([0, 0, 0, 0]), pathMetadata: new Int32Array([1, 0]) }, camera, viewport, atlas);
    const railing = scene.triangles.find((triangle) => triangle.kind === "path-railing");
    expect(railing).toBeDefined();
    const x = railing!.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 3;
    const y = railing!.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 3;

    expect(pickScene(scene, x, y)).toMatchObject({ kind: "path", x: 0, y: 0 });
  });
  test("picks the owning path at the outer top edge of a queue railing", () => {
    const scene = buildScene({ ...world, paths: new Int32Array([0, 0, 0, 0]), pathMetadata: new Int32Array([1, 0]) }, camera, viewport, atlas);
    const railingTriangles = scene.triangles.filter((triangle) => triangle.kind === "path-railing");
    const pathTriangles = scene.triangles.filter((triangle) => triangle.kind === "path" && triangle.tileX === 0 && triangle.tileY === 0);
    const contains = (triangle: typeof scene.triangles[number], x: number, y: number): boolean => {
      const [a, b, c] = triangle.vertices;
      const denominator = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
      const first = ((b.y - c.y) * (x - c.x) + (c.x - b.x) * (y - c.y)) / denominator;
      const second = ((c.y - a.y) * (x - c.x) + (a.x - c.x) * (y - c.y)) / denominator;
      const third = 1 - first - second;
      return first >= 0 && second >= 0 && third >= 0;
    };
    let exposedRailPoint: { x: number; y: number } | undefined;
    for (const triangle of railingTriangles) {
      const [a, b, c] = triangle.vertices;
      for (let first = 0.1; first < 0.9; first += 0.1) {
        for (let second = 0.1; first + second < 0.9; second += 0.1) {
          const third = 1 - first - second;
          const point = {
            x: first * a.x + second * b.x + third * c.x,
            y: first * a.y + second * b.y + third * c.y,
          };
          if (!pathTriangles.some((path) => contains(path, point.x, point.y))) exposedRailPoint = point;
        }
      }
    }

    expect(exposedRailPoint).toBeDefined();
    expect(pickScene(scene, exposedRailPoint!.x, exposedRailPoint!.y)).toMatchObject({ kind: "path", x: 0, y: 0, height: 0 });
  });
  test("path placement ghosts do not intercept world picking", () => {
    const scene = buildScene({ ...world, paths: new Int32Array([0, 0, 0, 0]) }, camera, viewport, atlas);
    const ghost = buildPathGhostQuads([{ x: 0, y: 0, z: 0, slope: 0, refusal: null }], camera, viewport);
    appendGeometry(scene, ghost);
    const triangle = scene.triangles.find((item) => item.kind === "path-preview")!;
    const x = triangle.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 3;
    const y = triangle.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 3;

    expect(pickScene(scene, x, y)).toMatchObject({ kind: "path", x: 0, y: 0 });
  });
  test("hides paths beneath terrain", () => {
    const surfaces = new Int32Array(world.surfaces);
    surfaces.set([8, 8, 8, 8, 0, 0]);
    const scene = buildScene({ ...world, surfaces, paths: new Int32Array([0, 0, 4, 0]) }, camera, viewport, atlas);
    expect(scene.triangles.some((triangle) => triangle.kind === "path")).toBe(false);
  });
  test("shows an underground path through a carved opening at the surface", () => {
    const surfaces = new Int32Array(world.surfaces);
    surfaces.set([8, 8, 8, 8, 0, 0]);
    const open = buildScene({ ...world, surfaces, paths: new Int32Array([0, 0, 4, 0]), carved: new Int32Array([0, 0, 4, 8]) }, camera, viewport, atlas);
    expect(open.triangles.some((triangle) => triangle.kind === "path" && triangle.tileX === 0 && triangle.tileY === 0)).toBe(true);
    expect(open.triangles.some((triangle) => triangle.kind === "terrain" && triangle.tileX === 0 && triangle.tileY === 0)).toBe(false);
    const covered = buildScene({ ...world, surfaces, paths: new Int32Array([0, 0, 4, 0]), carved: new Int32Array([0, 0, 4, 7]) }, camera, viewport, atlas);
    expect(covered.triangles.some((triangle) => triangle.kind === "path")).toBe(false);
  });
  test("draws the exposed walls around a surface opening in every rotation", () => {
    const surfaces = new Int32Array(3 * 3 * 6);
    for (let index = 0; index < surfaces.length; index += 6) surfaces.set([8, 8, 8, 8, 0, 0], index);
    const openingWorld: WorldView = { dimensions: new Uint16Array([3, 3]), surfaces, paths: new Int32Array(), carved: new Int32Array([1, 1, 4, 8]) };
    for (const rotation of [0, 1, 2, 3] as const) {
      const scene = buildScene(openingWorld, { ...camera, rotation }, viewport, atlas);
      const walls = scene.triangles.filter((triangle) => triangle.kind === "cliff" && triangle.tileX > 0 && triangle.tileX < 2 || triangle.kind === "cliff" && triangle.tileY > 0 && triangle.tileY < 2);
      const interiorWalls = walls.filter((triangle) => triangle.vertices.every((vertex) => vertex.height >= 4 && vertex.height <= 8));
      expect(interiorWalls).toHaveLength(16);
      expect(new Set(interiorWalls.map((triangle) => Math.min(...triangle.vertices.map((vertex) => vertex.height))))).toEqual(new Set([4, 5, 6, 7]));
    }
  });
  test("draws cliffs from the opening floor beside lower terrain and at map edges", () => {
    const surfaces = new Int32Array(3 * 3 * 6);
    surfaces.set([8, 8, 8, 8, 0, 0], 4 * 6);
    const openingWorld: WorldView = { dimensions: new Uint16Array([3, 3]), surfaces, paths: new Int32Array(), carved: new Int32Array([1, 1, 4, 8]) };
    const edgeWorld: WorldView = { dimensions: new Uint16Array([1, 1]), surfaces: new Int32Array([8, 8, 8, 8, 0, 0]), paths: new Int32Array(), carved: new Int32Array([0, 0, 4, 8]) };
    for (const rotation of [0, 1, 2, 3] as const) {
      const cliffs = buildScene(openingWorld, { ...camera, rotation }, viewport, atlas).triangles.filter((triangle) => triangle.kind === "cliff" && triangle.tileX === 1 && triangle.tileY === 1);
      expect(cliffs).toHaveLength(16);
      expect(Math.max(...cliffs.flatMap((triangle) => triangle.vertices.map((vertex) => vertex.height)))).toBe(4);
      const edgeCliffs = buildScene(edgeWorld, { ...camera, rotation }, viewport, atlas).triangles.filter((triangle) => triangle.kind === "cliff");
      expect(edgeCliffs).toHaveLength(4);
      expect(Math.max(...edgeCliffs.flatMap((triangle) => triangle.vertices.map((vertex) => vertex.height)))).toBe(4);
    }
  });
  test("draws the height difference between adjacent surface openings", () => {
    const surfaces = new Int32Array(3 * 3 * 6);
    for (let index = 0; index < surfaces.length; index += 6) surfaces.set([8, 8, 8, 8, 0, 0], index);
    const openingWorld: WorldView = { dimensions: new Uint16Array([3, 3]), surfaces, paths: new Int32Array(), carved: new Int32Array([1, 1, 6, 8, 1, 2, 4, 8]) };
    const cliffs = buildScene(openingWorld, camera, viewport, atlas).triangles.filter((triangle) => triangle.kind === "cliff" && triangle.tileX === 1 && triangle.tileY === 1);
    expect(cliffs).toHaveLength(4);
    expect(new Set(cliffs.flatMap((triangle) => triangle.vertices.map((vertex) => vertex.height)))).toEqual(new Set([4, 5, 6]));
  });
  test("culls a large map while retaining elevated visible tiles", () => {
    const surfaces = new Int32Array(512 * 512 * 6);
    const tile = (256 * 512 + 256) * 6;
    surfaces.set([12, 12, 12, 12, 0, 0], tile);
    const scene = buildScene({ dimensions: new Uint16Array([512, 512]), surfaces, paths: new Int32Array() }, { ...camera, centerX: 256, centerY: 256 }, viewport, atlas);
    expect(scene.triangles.length).toBeLessThan(1000);
    expect(scene.triangles.some((triangle) => triangle.tileX === 256 && triangle.tileY === 256 && triangle.kind === "terrain")).toBe(true);
  });
  test("reflects surface mutations in every rotation", () => {
    const surfaces = new Int32Array(6);
    const mutableWorld: WorldView = { dimensions: new Uint16Array([1, 1]), surfaces, paths: new Int32Array() };
    for (const rotation of [0, 1, 2, 3] as const) {
      surfaces.set([0, 0, 0, 0, 0, 0]);
      const flat = buildScene(mutableWorld, { ...camera, rotation }, viewport, atlas);
      expect(flat.triangles.some((triangle) => triangle.kind === "water")).toBe(false);
      surfaces.set([0, 0, 0, 0, 1, 4]);
      const flooded = buildScene(mutableWorld, { ...camera, rotation }, viewport, atlas);
      expect(flooded.triangles.some((triangle) => triangle.kind === "water" && triangle.height === 4)).toBe(true);
    }
  });
  test("retains a visible elevated path when its terrain is off screen", () => {
    const elevatedWorld: WorldView = {
      dimensions: new Uint16Array([12, 12]),
      surfaces: new Int32Array(12 * 12 * 6),
      paths: new Int32Array([10, 10, 160, 0]),
    };
    const scene = buildScene(elevatedWorld, { ...camera, centerX: 0, centerY: 0 }, { width: 100, height: 100 }, atlas);
    expect(scene.triangles.some((triangle) => triangle.kind === "path" && triangle.tileX === 10 && triangle.tileY === 10)).toBe(true);
  });
  test("rejects malformed snapshots and missing atlas frames", () => {
    expect(() => buildScene({ ...world, surfaces: new Int32Array() }, camera, viewport, atlas)).toThrow();
    expect(() => buildScene(world, camera, viewport, { ...atlas, frames: {} })).toThrow();
  });
});
