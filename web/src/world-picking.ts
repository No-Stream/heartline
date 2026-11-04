import { projectWorld, type Camera, type Viewport } from "./camera";
import type { ScreenPoint } from "./input/actions";
import type { LandscapingTile, TerrainCorner } from "./landscaping-tool";
import type { Pick, WorldView } from "./render/scene";
import type { SceneryTile } from "./scenery-tool";

const LANDSCAPING_CORNERS: readonly TerrainCorner[] = ["north_west", "north_east", "south_east", "south_west"];
const LANDSCAPING_OFFSETS = [[0, 0], [1, 0], [1, 1], [0, 1]] as const;
const SCENERY_QUARTERS = [[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]] as const;

/** Resolve which terrain corner the pointer is closest to, matching the renderer's surface pick. */
export function landscapingTileFromPick(
  pick: Pick | null,
  world: WorldView | undefined,
  position: ScreenPoint | null,
  camera: Camera,
  viewport: Viewport,
  undergroundLevel?: number,
): LandscapingTile | null {
  if (!pick || !world || !position || (pick.kind !== "terrain" && pick.kind !== "carved-floor" && pick.kind !== "path")) return null;

  const surfaceOffset = (pick.y * world.dimensions[0] + pick.x) * 6;
  const distances = LANDSCAPING_OFFSETS.map(([dx, dy], index) => {
    const height = pick.kind === "terrain" && undergroundLevel === undefined
      ? world.surfaces[surfaceOffset + index]
      : pick.height;
    const point = projectWorld(camera, viewport, { x: pick.x + dx, y: pick.y + dy, z: height / 4 });
    return Math.hypot(point.x - position.x, point.y - position.y);
  });
  const corner = distances.indexOf(Math.min(...distances));
  return {
    x: pick.x,
    y: pick.y,
    ...(undergroundLevel !== undefined ? { height: Math.round(pick.height) } : {}),
    corner: LANDSCAPING_CORNERS[corner],
  };
}

/** Resolve the quarter-tile scenery cell nearest the pointer on a terrain tile. */
export function sceneryTileFromPick(
  pick: Pick | null,
  position: ScreenPoint | null,
  camera: Camera,
  viewport: Viewport,
): SceneryTile | null {
  if (!pick || !position || pick.kind !== "terrain") return null;

  const distances = SCENERY_QUARTERS.map(([dx, dy]) => {
    const point = projectWorld(camera, viewport, { x: pick.x + dx, y: pick.y + dy, z: pick.height / 4 });
    return Math.hypot(point.x - position.x, point.y - position.y);
  });
  return { x: pick.x, y: pick.y, z: Math.round(pick.height), quarter: distances.indexOf(Math.min(...distances)) };
}

/** Return the suffix from a renderer ID such as `scenery-12:4:8:1`. */
export function entityIdFromPick(pick: Pick | null, prefix: string): string | undefined {
  if (!pick || !("id" in pick)) return undefined;
  const match = new RegExp(`^${prefix}-(.+)$`).exec(pick.id);
  return match?.[1];
}

export function facilityIdFromPick(pick: Pick | null): number | undefined {
  if (!pick || !("id" in pick)) return undefined;
  const match = /^facility-(\d+)$/.exec(pick.id);
  return match ? Number(match[1]) : undefined;
}

/** Resolve a placement tile from the ground scene while retaining building occlusion. */
export function facilityTileFromPicks(surfacePick: Pick | null, foregroundPick: Pick | null): { readonly x: number; readonly y: number; readonly height: number } | null {
  const isFacilitySprite = !!foregroundPick && "id" in foregroundPick && foregroundPick.kind === "sprite" && foregroundPick.id.startsWith("facility-");
  if (foregroundPick?.kind === "facility" || foregroundPick?.kind === "building" || isFacilitySprite) return null;
  if (!surfacePick || (surfacePick.kind !== "terrain" && surfacePick.kind !== "path" && surfacePick.kind !== "carved-floor")) return null;
  return { x: surfacePick.x, y: surfacePick.y, height: surfacePick.height };
}

/** Resolve a station tile from terrain or a path, ignoring all rendered overlays. */
export function stationTileFromSurfacePick(pick: Pick | null): { readonly x: number; readonly y: number; readonly height: number } | null {
  if (!pick || (pick.kind !== "terrain" && pick.kind !== "path" && pick.kind !== "carved-floor")) return null;
  return { x: pick.x, y: pick.y, height: pick.height };
}

export function blockBoundaryIndexFromPick(pick: Pick | null): number | null {
  if (!pick || !("id" in pick)) return null;
  const match = /^block-boundary-(\d+)$/.exec(pick.id);
  return match ? Number(match[1]) : null;
}
