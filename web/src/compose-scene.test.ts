import { composeScene, type ComposeSceneInput, type SceneWorld } from "./compose-scene";
import { describe, expect, it } from "vitest";
import type { Atlas } from "./render/scene";
import type { Facility } from "./facilities";
import type { Scenery } from "./scenery-tool";

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
});
