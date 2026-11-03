import type { PathTile } from "./path-tool";
import type { Pick, WorldView } from "./render/scene";
import { pathCornerHeights } from "./render/path-ghost";

const pathHeightTolerance = 0.001;

/** Resolve a renderer pick to the integer quarter-step height path commands require. */
export function pathTileFromPick(pick: Pick | null, world: WorldView): PathTile | null {
  if (!pick || (pick.kind !== "terrain" && pick.kind !== "path")) return null;
  const [width, height] = world.dimensions;
  if (!width || !height || world.surfaces.length !== width * height * 6 || world.paths.length % 4 !== 0) {
    throw new Error("Invalid world snapshot for path placement");
  }
  if (!Number.isInteger(pick.x) || !Number.isInteger(pick.y) || pick.x < 0 || pick.y < 0 || pick.x >= width || pick.y >= height || !Number.isFinite(pick.height)) {
    throw new RangeError("Invalid world pick for path placement");
  }

  if (pick.kind === "terrain") {
    const surfaceOffset = (pick.y * width + pick.x) * 6;
    const heightSteps = Math.min(...world.surfaces.subarray(surfaceOffset, surfaceOffset + 4));
    return { x: pick.x, y: pick.y, z: heightSteps, kind: "terrain" };
  }

  const matchingPaths: number[] = [];
  for (let offset = 0; offset < world.paths.length; offset += 4) {
    const [x, y, baseHeight, slope] = world.paths.subarray(offset, offset + 4);
    if (x !== pick.x || y !== pick.y) continue;
    const cornerHeights = pathCornerHeights(baseHeight, slope);
    const lowestHeight = Math.min(...cornerHeights);
    const highestHeight = Math.max(...cornerHeights);
    if (pick.height >= lowestHeight - pathHeightTolerance && pick.height <= highestHeight + pathHeightTolerance) {
      matchingPaths.push(baseHeight);
    }
  }
  if (matchingPaths.length !== 1) return null;
  return { x: pick.x, y: pick.y, z: matchingPaths[0], kind: "path" };
}
