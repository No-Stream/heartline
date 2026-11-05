import { projectWorld, type Camera, type Viewport } from "./camera";
import type { CoasterColorScheme } from "./coaster-colors";
import type { ConstructionState } from "./construction-protocol";
import { buildFacilityGeometry, facilitySprites, type Facility } from "./facilities";
import { buildConfetti, buildFireworks, buildLights, buildShadows, buildSky, parkBounds, type CrestBurst } from "./render/effects";
import { BACKDROPS, DAY_LENGTH_TICKS, lightingAt, printOnBackdrop, sunAt } from "./render/daylight";
import { buildPeople, riderAppearances } from "./render/people";
import { buildTrackQuads, interpolateTrainPoses, trackPaint, trackStyle, type TrackQuad } from "./render/track/geometry";
import { buildRiderQuads, buildTrainQuads } from "./render/track/trains";
import type { TrainLayout } from "./train-layout";
import { buildGhostQuads } from "./render/track/ghost";
import { buildBlockBoundaryQuads } from "./render/track/blocks";
import { buildSelectionRing } from "./guest-panel/selection-ring";
import { buildPathGhostQuads, decodePathPreview } from "./render/path-ghost";
import { buildLandscapingGhostQuads, type LandscapingPreview } from "./landscaping-tool";
import { buildSceneryGhostQuads, scenerySprites, type Scenery, type SceneryPreview } from "./scenery-tool";
import { buildProximityMarkers, buildTestingRibbon, overlaySamples, type TestingMetric } from "./coaster-testing";
import type { Atlas, GeometryQuad, WorldSprite, WorldView } from "./render/scene";
import type { AtmospherePass, GeometryPass } from "./render/index";
import type { CoasterSnapshot } from "./protocol";
import type { PathToolPreview } from "./path-tool";
import type { Theme } from "./theme";
import type { Tool } from "./toolbar";

export interface SceneWorld extends WorldView {
  readonly coasterRails: Float32Array;
  readonly coasterTies: Float32Array;
  readonly coasterSupports: Float32Array;
  readonly coasterColors: CoasterColorScheme | null;
  readonly coasterTrain: TrainLayout | null;
}

export interface RenderedRiderBounds {
  readonly id: string;
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}

export interface SceneComposition {
  readonly view: WorldView & { readonly trainPoses: Float32Array };
  readonly camera: Camera;
  readonly waterFrame: number;
  readonly geometryPasses: readonly GeometryPass[];
  readonly atmospherePass: AtmospherePass;
  readonly guests: readonly WorldSprite[];
  readonly riders: RenderedRiderBounds[];
}

export interface ComposeSceneInput {
  readonly world: SceneWorld;
  readonly previous?: CoasterSnapshot;
  readonly current?: CoasterSnapshot;
  readonly interpolation: number;
  readonly camera: Camera;
  readonly screenCamera: Camera;
  readonly viewport: Viewport;
  readonly atlas: Atlas;
  readonly facilities: readonly Facility[];
  readonly scenery: readonly Scenery[];
  readonly undergroundLevel?: number;
  readonly construction?: ConstructionState;
  readonly activeTool: Tool;
  readonly selectedGuestId?: string;
  readonly sceneryPreview: SceneryPreview | null;
  readonly landscapingPreview: LandscapingPreview | null;
  readonly pathPreview: PathToolPreview | null;
  readonly stationGhost: () => Float32Array;
  readonly testingMetric: TestingMetric | null;
  readonly testingHighlightDistance?: number;
  readonly constructionEffects: { quads(now: number, camera: Camera, viewport: Viewport): GeometryQuad[] };
  readonly staticTrack: (rails: Float32Array, ties: Float32Array, supports: Float32Array, colors: CoasterColorScheme, camera: Camera, viewport: Viewport) => readonly TrackQuad[];
  readonly builtTrack: (rails: Float32Array, ties: Float32Array, supports: Float32Array, colors: CoasterColorScheme, camera: Camera, viewport: Viewport) => readonly TrackQuad[];
  readonly now: number;
  readonly frozen: boolean;
  readonly dayPhase: number;
  readonly theme: Theme;
  readonly crestBursts: readonly CrestBurst[];
  readonly renderTick?: number;
  readonly setGuestAnchor: (point: { x: number; y: number }) => void;
}

const noGeometry = new Float32Array();

/** Assemble the world view and every render pass from the state available at the start of a frame. */
export function composeScene(input: ComposeSceneInput): SceneComposition {
  const previewWouldTunnel = input.construction?.pieces.find((piece) => piece.id === input.construction?.selected)?.wouldTunnel
    ?? input.construction?.elements.find((element) => element.id === input.construction?.selected)?.wouldTunnel
    ?? false;
  const occupants = input.current ? riderAppearances(input.current, input.current.constructionRiders, input.atlas) : new Map();
  const ridingIds = new Set([...occupants.values()].map((occupant) => occupant.id));
  const people = input.current
    ? buildPeople(input.previous, input.current, input.interpolation, input.camera.rotation, input.atlas).filter((person) => !ridingIds.has(person.id))
    : [];
  const selectedGuest = people.find((guest) => guest.id === input.selectedGuestId);
  if (selectedGuest) {
    input.setGuestAnchor(projectWorld(input.screenCamera, input.viewport, {
      x: selectedGuest.x,
      y: selectedGuest.y,
      z: selectedGuest.height / 4,
    }));
  }

  const trainPoses = input.current
    ? interpolateTrainPoses(input.previous?.trainPoses ?? input.current.trainPoses, input.current.trainPoses, input.interpolation)
    : noGeometry;
  const constructionTrainPoses = input.current
    ? interpolateTrainPoses(input.previous?.constructionTrainPoses ?? input.current.constructionTrainPoses, input.current.constructionTrainPoses, input.interpolation)
    : noGeometry;
  const testingSamples = overlaySamples(
    input.construction?.testingReport?.samples ?? null,
    input.construction?.constructionForcePreview?.samples ?? null,
  );
  const sprites = [
    ...facilitySprites(input.facilities, input.camera.rotation),
    ...scenerySprites(input.scenery, input.camera.rotation),
    ...people,
  ];
  const view = { ...input.world, sprites, trainPoses, undergroundLevel: input.undergroundLevel };
  const effectTick = input.frozen ? Math.round(input.dayPhase * DAY_LENGTH_TICKS) : input.renderTick ?? 0;
  const backdrop = BACKDROPS[input.theme];
  const { lighting, paper } = printOnBackdrop(lightingAt(input.dayPhase), backdrop);
  const sun = sunAt(input.dayPhase);
  const atmospherePass: AtmospherePass = (camera, viewport, atlas, scale) => {
    const rails = input.construction ? [input.world.coasterRails, input.construction.placedRails] : [input.world.coasterRails];
    const lights = buildLights({ facilities: input.facilities, rails, sprites, atlas, scale }, lighting, effectTick, camera, viewport);
    const fireworks = buildFireworks(parkBounds(input.facilities, rails), lighting, effectTick, camera, viewport);
    const horizonY = projectWorld(camera, viewport, {
      x: input.world.dimensions[0] / 2,
      y: input.world.dimensions[1] / 2,
      z: 0,
    }).y;
    return {
      lighting,
      horizonY,
      paper,
      sky: buildSky(lighting, backdrop, input.dayPhase, horizonY, camera, viewport, atlas, scale),
      decals: buildShadows(input.facilities, lighting, sun, camera, viewport),
      lights: lights.bulbs,
      pools: lights.pools,
      overlay: buildConfetti(input.crestBursts, effectTick, camera, viewport),
      sparks: fireworks.sparks,
      glows: [...lights.glows, ...fireworks.glows],
    };
  };

  const riders: RenderedRiderBounds[] = [];
  const geometryPasses: readonly GeometryPass[] = [
    (camera, viewport) => buildSceneryGhostQuads(input.sceneryPreview, view, camera, viewport),
    (camera, viewport) => buildLandscapingGhostQuads(input.landscapingPreview, view, camera, viewport),
    (camera, viewport) => input.pathPreview ? buildPathGhostQuads(decodePathPreview(input.pathPreview.paths), camera, viewport) : [],
    (camera, viewport, atlas) => buildFacilityGeometry(input.facilities, camera, viewport, trackStyle(atlas)),
    (camera, viewport, atlas) => buildSelectionRing(selectedGuest, camera, viewport, atlas),
    (camera, viewport, atlas, scale) => [
      ...(input.world.coasterColors ? input.staticTrack(input.world.coasterRails, input.world.coasterTies, input.world.coasterSupports, input.world.coasterColors, camera, viewport) : []),
      ...(input.construction?.colors ? (input.activeTool === "coaster" && input.construction.blockReport.sections.length
        ? buildTrackQuads(input.construction.placedRails, input.construction.placedTies, input.construction.placedSupports, camera, viewport, trackPaint(trackStyle(atlas), input.construction.colors), input.construction.blockReport.sections.map((section) => section.boundaryMetres), input.construction.railDistances)
        : input.builtTrack(input.construction.placedRails, input.construction.placedTies, input.construction.placedSupports, input.construction.colors, camera, viewport)).map((quad) => ({ ...quad, id: `construction-${quad.id}` })) : []),
      ...(input.construction && input.activeTool === "coaster" ? buildBlockBoundaryQuads(input.construction.placedRails, input.construction.railDistances, input.construction.blockReport.sections, camera, viewport, trackStyle(atlas)) : []),
      ...buildGhostQuads(input.stationGhost(), noGeometry, noGeometry, true, camera, viewport, trackStyle(atlas)).map((quad) => ({ ...quad, color: [0.25, 0.55, 0.85, 0.55] as const })),
      ...(input.construction ? buildGhostQuads(input.construction.previewRails, input.construction.previewTies, input.construction.previewSupports, input.construction.canAppend, camera, viewport, trackStyle(atlas), previewWouldTunnel) : []),
      ...(input.world.coasterColors && input.world.coasterTrain ? buildTrainQuads(trainPoses, camera, viewport, trackPaint(trackStyle(atlas), input.world.coasterColors), input.world.coasterTrain) : []),
      ...(input.construction?.colors && input.construction.trainLayout ? buildTrainQuads(constructionTrainPoses, camera, viewport, trackPaint(trackStyle(atlas), input.construction.colors), input.construction.trainLayout) : []),
      ...(input.world.coasterTrain ? buildRiderQuads(trainPoses, camera, viewport, atlas, scale, input.world.coasterTrain) : []),
      ...(input.construction?.trainLayout ? buildRiderQuads(constructionTrainPoses, camera, viewport, atlas, scale, input.construction.trainLayout, occupants) : []).map((quad) => {
        riders.push({
          id: quad.id,
          left: Math.min(...quad.vertices.map((vertex) => vertex.x)),
          right: Math.max(...quad.vertices.map((vertex) => vertex.x)),
          top: Math.min(...quad.vertices.map((vertex) => vertex.y)),
          bottom: Math.max(...quad.vertices.map((vertex) => vertex.y)),
        });
        if (quad.id === input.selectedGuestId) {
          input.setGuestAnchor({
            x: quad.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 4,
            y: quad.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 4,
          });
        }
        return quad;
      }),
      ...(testingSamples.length && (input.testingMetric || input.testingHighlightDistance !== undefined)
        ? buildTestingRibbon(
          testingSamples,
          input.testingMetric ?? "speed",
          camera,
          viewport,
          trackStyle(atlas),
          input.testingHighlightDistance,
          input.construction?.constructionForcePreview?.limits,
        )
        : []),
      ...(input.construction?.testingReport ? buildProximityMarkers(input.construction.testingReport.proximityEvents, camera, viewport, trackStyle(atlas)) : []),
      ...input.constructionEffects.quads(input.now, camera, viewport),
    ],
  ];

  return {
    view,
    camera: input.screenCamera,
    waterFrame: input.frozen ? 0 : Math.floor(input.now / 250) % 4,
    geometryPasses,
    atmospherePass,
    guests: people,
    riders,
  };
}
