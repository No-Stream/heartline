import { composeScene, type ComposeSceneInput, type SceneWorld } from "./compose-scene";
import { describe, expect, it } from "vitest";
import type { Atlas } from "./render/scene";
import type { Facility } from "./facilities";
import type { Scenery } from "./scenery-tool";
import type { CoasterRenderData, CoasterSnapshot } from "./protocol";
import type { ConstructionState } from "./construction-protocol";
import type { TestingSample } from "./generated/TestingSample";
import { buildTrackQuads, trackPaint } from "./render/track/geometry";
import { buildTrainQuads } from "./render/track/trains";

const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
const viewport = { width: 400, height: 300 };
const atlas: Atlas = {
  themeStyle: { steel: "#3f61a6", wood: "#9b563b", train: "#dc5c3e", ink: "#3a2e58", lineWeight: 1 },
  pages: {},
  frames: {},
};
const personFrame = { width: 1, height: 1, anchor: { x: 0, y: 0 }, placements: {} };
const atlasWithPerson: Atlas = {
  ...atlas,
  frames: { "people/maya/se/idle/0": personFrame },
  sprites: { "people/maya": { appearance: { skinToneGroup: "deep", hairGroup: "curly", clothingPaletteGroup: "red", bodyShape: "slender", ageGroup: "child" } } },
} as Atlas;

const world: SceneWorld = {
  dimensions: new Uint16Array([4, 4]),
  surfaces: new Int32Array(4 * 4 * 6),
  paths: new Int32Array(),
  coasters: [],
  editingCoasterId: null,
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
    const forceEnvelopeSamples = [
      { distance: 0, x: 1, y: 1, z: 1, speed: 4, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
      { distance: 1, x: 2, y: 1, z: 1, speed: 8, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
    ];
    const construction = { constructionForcePreview: { samples: [], forceEnvelopeSamples }, testingReport: null, pieces: [], elements: [], previewRails: new Float32Array(), previewTies: new Float32Array(), previewSupports: new Float32Array() } as unknown as ConstructionState;
    const scene = composeScene(sceneInput({ construction, testingMetric: "speed" }));
    expect(scene.geometryPasses[5](camera, viewport, atlas, "1").some((quad) => quad.id === "testing-0")).toBe(true);
  });
  it("colours the selected candidate track by its force limit closeness before placement", () => {
    const samples = [
      { distance: 20, x: 1, y: 1, z: 1, speed: 16, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
      { distance: 21, x: 2, y: 1, z: 1, speed: 16, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
    ];
    const forceEnvelopeSamples = [
      samples[0],
      { ...samples[1], verticalG: 4.1 },
    ];
    const limits = {
      minimumVerticalG: -1.5,
      maximumVerticalG: 4.5,
      maximumLateralG: 1.8,
      maximumLongitudinalG: 1.5,
      ratingBands: {},
    };
    const construction = {
      candidateForcePreview: { samples, forceEnvelopeSamples, limits },
      canAppend: true,
      previewRails: new Float32Array(),
      previewTies: new Float32Array(),
      previewSupports: new Float32Array(),
      blockReport: { sections: [] },
      placedRails: new Float32Array(),
      railDistances: new Float32Array(),
      pieces: [],
      elements: [],
    } as unknown as ConstructionState;
    const scene = composeScene(sceneInput({ construction, activeTool: "coaster" }));
    const candidateRibbon = scene.geometryPasses[5](camera, viewport, atlas, "1")
      .find((quad) => quad.id === "candidate-force-0");

    expect(candidateRibbon?.color).toEqual([242 / 255, 181 / 255, 58 / 255, 1]);
  });
  it("highlights the ride window's point on the track without any builder report", () => {
    const samples = [0, 1, 2].map((distance): TestingSample => ({ distance, x: 1 + distance, y: 2, z: 0, speed: 10, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] }));
    const quads = composeScene(sceneInput({ rideHighlight: { samples, distance: 1.25 } })).geometryPasses[5](camera, viewport, atlas, "1");
    expect(quads.find(({ id }) => id === "testing-1")?.color).toEqual([58 / 255, 46 / 255, 88 / 255, 1]);
    expect(composeScene(sceneInput({})).geometryPasses[5](camera, viewport, atlas, "1").some(({ id }) => id.startsWith("testing-"))).toBe(false);
  });
  it("uses the world's train layout to render the model and its riders", () => {
    const frame = { width: 40, height: 60, anchor: { x: 20, y: 50 }, placements: { "1": { page: 0, x: 0, y: 0, width: 40, height: 60 } } };
    const riderAtlas: Atlas = { ...atlas, pages: { "1": [{ file: "test.png", width: 40, height: 60 }] }, frames: Object.fromEntries(["maya", "otis", "priya", "gus"].map((name) => [`people/${name}/se/idle/0`, { ...frame, properties: { "walk-cycle-length": "1" } }])), sprites: Object.fromEntries(["maya", "otis", "priya", "gus"].map((name) => [`people/${name}`, { appearance: { skinToneGroup: "deep", hairGroup: "curly", clothingPaletteGroup: "red", bodyShape: "slender", ageGroup: "child" } }])) } as Atlas;
    const snapshot: CoasterSnapshot = { tickCount: 0, guestIds: new Uint32Array([7]), guests: new Float32Array([1, 2, 0, 1, 0, 0, 0]), appearances: new Uint8Array([0, 0, 0, 0, 0]), coasters: [{ id: 7, trainPoses: new Float32Array([0, 1, 2, 3, 1, 0, 0, 0, 0, 1, 0]), trainStates: new Float32Array(), riders: new Uint32Array([0, 0, 7, 0, 1, 7]) }] };
    const layout = { model: "wooden_twister" as const, trackMaterial: "wood" as const, carsPerTrain: 12, rowsPerCar: 1, seatsPerRow: 2, lengthMetres: 18 };
    const coaster: CoasterRenderData = { id: 7, trackRevision: "revision-1", geometry: new Float32Array(), rails: new Float32Array(), ties: new Float32Array(), supports: new Float32Array(), colors: { track: "teal", supports: "cream", trains: "plum" }, trainLayout: layout };
    const scene = composeScene(sceneInput({ atlas: riderAtlas, current: snapshot, world: { ...world, coasters: [coaster] } }));
    const quads = scene.geometryPasses[5](camera, viewport, riderAtlas, "1");
    expect(quads.some((quad) => quad.id === "train-7-0")).toBe(true);
    expect(quads.filter((quad) => quad.kind === "rider").map((quad) => quad.id)).toEqual(["guest-7", "guest-7"]);
  });
  it("draws two coasters with separate colours, train poses and pick IDs", () => {
    const colors = [
      { track: "teal", supports: "cream", trains: "plum" },
      { track: "coral", supports: "timber", trains: "sage" },
    ] as const;
    const rails = new Float32Array([5, 1, 2, 1, 0, 1, 0, 0, 0, 1, 3, 2, 1, 0, 1, 0, 0, 0, 1]);
    const ties = new Float32Array();
    const supports = new Float32Array();
    const layout = { model: "hybrid" as const, trackMaterial: "hybrid" as const, carsPerTrain: 1, rowsPerCar: 1, seatsPerRow: 2, lengthMetres: 4 };
    const coasters = [
      { id: 7, trackRevision: "1", geometry: new Float32Array(), rails, ties, supports, colors: colors[0], trainLayout: layout },
      { id: 8, trackRevision: "1", geometry: new Float32Array(), rails, ties, supports, colors: colors[1], trainLayout: layout },
    ];
    const poses = (x: number) => new Float32Array([0, x, 2, 0, 1, 0, 0, 0, 0, 1, 0]);
    const snapshot = {
      tickCount: 1,
      guestIds: new Uint32Array(),
      guests: new Float32Array(),
      appearances: new Uint8Array(),
      coasters: [
        { id: 7, trainPoses: poses(1), trainStates: new Float32Array(), riders: new Uint32Array() },
        { id: 8, trainPoses: poses(3), trainStates: new Float32Array(), riders: new Uint32Array() },
      ],
    } as unknown as CoasterSnapshot;
    const scene = composeScene(sceneInput({
      atlas: atlasWithPerson,
      world: { ...world, coasters, editingCoasterId: null } as unknown as SceneWorld,
      current: snapshot,
      staticTrack: (trackRails, trackTies, trackSupports, trackColors, trackCamera, trackViewport, coasterId) => buildTrackQuads(
        trackRails,
        trackTies,
        trackSupports,
        trackCamera,
        trackViewport,
        trackPaint({ steel: "#3f61a6", wood: "#9b563b", train: "#dc5c3e", ink: "#3a2e58", lineWeight: 1 }, trackColors),
        undefined,
        undefined,
        coasterId,
      ),
      builtTrack: () => [],
    }));
    const quads = scene.geometryPasses[5](camera, viewport, atlas, "1");
    const firstTrack = quads.find((quad) => quad.id === "track-7-5");
    const secondTrack = quads.find((quad) => quad.id === "track-8-5");
    const firstTrainColors = new Set(quads.filter((quad) => quad.id === "train-7-0").map((quad) => quad.color.join()));
    const secondTrainColors = new Set(quads.filter((quad) => quad.id === "train-8-0").map((quad) => quad.color.join()));

    expect(firstTrack?.color).not.toEqual(secondTrack?.color);
    expect(firstTrainColors).not.toEqual(secondTrainColors);
    expect(firstTrack).toBeDefined();
    expect(secondTrack).toBeDefined();
    expect(firstTrainColors.size).toBeGreaterThan(0);
    expect(secondTrainColors.size).toBeGreaterThan(0);
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

  it("passes a selected score location into the testing ribbon", () => {
    const construction = {
      pieces: [],
      elements: [],
      colors: { track: "teal", supports: "cream", trains: "plum" },
      canAppend: false,
      previewRails: new Float32Array(),
      previewTies: new Float32Array(),
      previewSupports: new Float32Array(),
      testingReport: {
        samples: [
          { distance: 0, x: 1, y: 2, z: 0, speed: 10, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
          { distance: 1, x: 2, y: 2, z: 0, speed: 12, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
          { distance: 2, x: 3, y: 2, z: 0, speed: 14, verticalG: 1, lateralG: 0, longitudinalG: 0, airtime: false, forceLimitExceeded: [false, false, false] },
        ],
        proximityEvents: [],
      },
    } as unknown as ConstructionState;

    const quads = composeScene(sceneInput({
      construction,
      testingMetric: "speed",
      testingHighlightDistance: 1.25,
    })).geometryPasses[5](camera, viewport, atlas, "1");

    const highlighted = quads.find(({ id }) => id === "testing-1");
    expect(highlighted).toBeDefined();
    expect(highlighted?.color).toEqual([58 / 255, 46 / 255, 88 / 255, 1]);
  });
});
