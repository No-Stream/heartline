import type { LandscapingPreview, LandscapingSelection } from "./landscaping-tool";
import type { PathTile } from "./path-tool";
import { cutawayLevelForTrack } from "./render/cutaway";
import { pathCornerHeights, decodePathPreview } from "./render/path-ghost";
import { terrainHeightAt, type WorldView } from "./render/scene";
import type { WorldPoint } from "./camera";

export type UndergroundCutawayMode = "auto" | "off" | "on";

export interface UndergroundCutawayTarget extends WorldPoint {
  readonly level: number;
  readonly belowTerrain: boolean;
}

const storageKey = "theme-park-sim.underground-cutaway";

export function parseStoredUndergroundCutawayMode(value: string | null): UndergroundCutawayMode | undefined {
  return value === "auto" || value === "off" || value === "on" ? value : undefined;
}

export function storedUndergroundCutawayMode(): UndergroundCutawayMode {
  try {
    return parseStoredUndergroundCutawayMode(localStorage.getItem(storageKey)) ?? "auto";
  } catch (error) {
    if (error instanceof DOMException) return "auto";
    throw error;
  }
}

export function storeUndergroundCutawayMode(mode: UndergroundCutawayMode): void {
  try {
    localStorage.setItem(storageKey, mode);
  } catch (error) {
    if (!(error instanceof DOMException)) throw error;
  }
}

export function constructionCutawayTarget(
  end: { readonly position: WorldPoint; readonly metres: number } | null,
): UndergroundCutawayTarget | null {
  if (!end) return null;
  return {
    ...end.position,
    level: cutawayLevelForTrack(end.position.z),
    belowTerrain: end.metres < 0,
  };
}

export function pathCutawayTarget(world: WorldView, paths: Int32Array | null): UndergroundCutawayTarget | null {
  if (!paths?.length) return null;
  const endpoint = decodePathPreview(paths).at(-1);
  if (!endpoint) return null;
  const { x, y, z, slope } = endpoint;
  const width = world.dimensions[0];
  const surfaceOffset = (y * width + x) * 6;
  const terrainCorners = world.surfaces.subarray(surfaceOffset, surfaceOffset + 4);
  const pathHeights = pathCornerHeights(z, slope);
  const highestPathHeight = Math.max(...pathHeights);
  return {
    x: x + 0.5,
    y: y + 0.5,
    z: highestPathHeight / 4,
    level: cutawayLevelForTrack(highestPathHeight / 4),
    belowTerrain: pathHeights.some((height, index) => height < terrainCorners[index]),
  };
}

export function pathTileCutawayTarget(world: WorldView, tile: PathTile | null): UndergroundCutawayTarget | null {
  if (!tile) return null;
  const belowTerrain = tile.z < terrainHeightAt(world, tile.x + 0.5, tile.y + 0.5);
  const z = tile.z / 4;
  return {
    x: tile.x + 0.5,
    y: tile.y + 0.5,
    z,
    level: cutawayLevelForTrack(z),
    belowTerrain,
  };
}

export function landscapingCutawayTarget(
  world: WorldView,
  preview: LandscapingPreview | null,
  selection: LandscapingSelection | null,
): UndergroundCutawayTarget | null {
  if (!preview || preview.tool !== "carve" || !selection) return null;
  const x = preview.x + preview.size / 2;
  const y = preview.y + preview.size / 2;
  const z = selection.bottom / 4;
  return {
    x,
    y,
    z,
    level: cutawayLevelForTrack(z),
    belowTerrain: selection.bottom < terrainHeightAt(world, x, y),
  };
}

export function shouldShowAutomaticCutaway(
  mode: UndergroundCutawayMode,
  target: UndergroundCutawayTarget | null,
): boolean {
  if (!target || mode === "off") return false;
  return mode === "on" || target.belowTerrain;
}
