import { composeScene, type ComposeSceneInput, type SceneWorld } from "./compose-scene";
import { describe, expect, it } from "vitest";
import type { Atlas } from "./render/scene";
import type { Facility } from "./facilities";
import type { Scenery } from "./scenery-tool";
import type { CoasterSnapshot } from "./protocol";
import type { ConstructionState } from "./construction-protocol";

const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
const viewport = { width: 400, height: 300 };
const atlas: Atlas = {
  themeStyle: { steel: "#3f61a6", wood: "#9b563b", train: "#dc5c3e", ink: "#3a2e58", lineWeight: 1 },
  pages: {},
  frames: {},
};

const world: SceneWorld = {
  dimensions: new Uint16Array([4, 4]),
  surfaces: new Int32Array(4 * 4 * 6),
  paths: new Int32Array(),
  coasterRails: new Float32Array(),
  coasterTies: new Float32Array(),
  coasterSupports: new Float32Array(),
  coasterColors: null,
  coasterTrain: null,
};

const facility: Facility = {
  id: 7,
  kind: "food_stall",
  x: 1,
  y: 2,
  z: 0,
  rotation: 0,
  price: 5,
  ride_state: "idle",
  ride_progress: 0,
};

const scenery: Scenery = {
  id: "lamp-1",
  kind: "lamp",
  x: 2,
  y: 1,
  z: 0,
  rotation: 1,
  quarter: 2,
};

function sceneInput(overrides: Partial<ComposeSceneInput> = {}): ComposeSceneInput {
  return {
    world,
    interpolation: 1,
    camera,
    screenCamera: camera,
    viewport,
    atlas,
    facilities: [facility],
    scenery: [scenery],
    undergroundLevel: 3,
    activeTool: "none",
    sceneryPreview: null,
    landscapingPreview: null,
    pathPreview: null,
    stationGhost: () => new Float32Array(),
    testingMetric: null,
    constructionEffects: { quads: () => [] },
    staticTrack: () => [],
    builtTrack: () => [],
    now: 510,
    frozen: false,
    dayPhase: 0.5,
    theme: "light",
    crestBursts: [],
    setGuestAnchor: () => {},
    ...overrides,
  };
}

describe("composeScene", () => {
  it("draws the live construction estimate as a pickable track ribbon", () => {
    const samples = [
      { distance: 0, x: 1, y: 1, z: 1, speed: 4, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false },
      { distance: 1, x: 2, y: 1, z: 1, speed: 8, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false },
    ];
    const construction = { constructionForcePreview: { samples }, testingReport: null, pieces: [], elements: [], previewRails: new Float32Array(), previewTies: new Float32Array(), previewSupports: new Float32Array() } as unknown as ConstructionState;
    const scene = composeScene(sceneInput({ construction, testingMetric: "speed" }));
    expect(scene.geometryPasses[5](camera, viewport, atlas, "1").some((quad) => quad.id === "testing-0")).toBe(true);
  });
  it("uses the world's train layout to render the model and its riders", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const riderAtlas: Atlas = { ...atlas, pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: Object.fromEntries(["maya", "otis", "priya", "gus"].map((name) => [`people/${name}/se/idle/0`, frame])), sprites: Object.fromEntries(["maya", "otis", "priya", "gus"].map((name) => [`people/${name}`, { appearance: { skinToneGroup: "deep", hairGroup: "curly", clothingPaletteGroup: "red", bodyShape: "slender", ageGroup: "child" } }])) } as Atlas;
    const snapshot: CoasterSnapshot = { tickCount: 0, guestIds: new Uint32Array(), guests: new Float32Array(), appearances: new Uint8Array(), trainPoses: new Float32Array([0, 1, 2, 3, 1, 0, 0, 0, 0, 1, 15]), constructionTrainPoses: new Float32Array(), constructionRiders: new Uint32Array() };
    const layout = { model: "wooden_twister" as const, carsPerTrain: 12, rowsPerCar: 1, seatsPerRow: 2, lengthMetres: 18 };
    const scene = composeScene(sceneInput({ atlas: riderAtlas, current: snapshot, world: { ...world, coasterTrain: layout, coasterColors: { track: "teal", supports: "cream", trains: "plum" } } }));
    const quads = scene.geometryPasses[5](camera, viewport, riderAtlas, "1");
    expect(quads.some((quad) => quad.id === "train-0")).toBe(true);
    expect(quads.filter((quad) => quad.id.startsWith("rider-")).map((quad) => quad.id)).toEqual(["rider-0-0", "rider-0-1"]);
  });
  it("assembles base world data, sprite layers and renderer passes from explicit frame state", () => {
    const scene = composeScene(sceneInput());

    expect(scene.view.dimensions).toBe(world.dimensions);
    expect(scene.view.undergroundLevel).toBe(3);
    expect(scene.view.sprites?.map(({ id }) => id)).toEqual(["facility-7", "scenery-lamp-1"]);
    expect(scene.view.trainPoses).toEqual(new Float32Array());
    expect(scene.waterFrame).toBe(2);
    expect(scene.geometryPasses).toHaveLength(6);

    const layers = scene.geometryPasses.map((pass) => pass(camera, viewport, atlas, "1"));
    expect(layers).toHaveLength(6);
    expect(layers.flat()).toEqual([]);
  });

  it("holds water animation at frame zero for frozen render state", () => {
    expect(composeScene(sceneInput({ now: 3_750, frozen: true })).waterFrame).toBe(0);
  });

  it("marks a selected construction piece that will tunnel", () => {
    const construction = {
      selected: "piece:down-gentle",
      pieces: [{ id: "piece:down-gentle", wouldTunnel: true }],
      elements: [],
      canAppend: true,
      previewRails: new Float32Array([0, 0, 0, 2, 0, 1, 0, 0, 0, 1, 1, 0, 2, 0, 1, 0, 0, 0, 1]),
      previewTies: new Float32Array(),
      previewSupports: new Float32Array(),
      placedRails: new Float32Array(),
      railDistances: new Float32Array(),
      placedTies: new Float32Array(),
      placedSupports: new Float32Array(),
      blockReport: { sections: [] },
    } as unknown as ConstructionState;
    const preview = composeScene(sceneInput({ construction, activeTool: "coaster" }))
      .geometryPasses[5](camera, viewport, atlas, "1")
      .filter((quad) => quad.id.startsWith("preview-"));

    expect(preview.length).toBeGreaterThan(0);
    expect(preview.every((quad) => quad.color[0] > quad.color[1] && quad.color[1] > quad.color[2])).toBe(true);
  });
});
