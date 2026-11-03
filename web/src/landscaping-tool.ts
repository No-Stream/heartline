import type { GeometryQuad, WorldView } from "./render/scene";
import { projectWorld, type Camera, type Viewport } from "./camera";

export const LANDSCAPING_TOOLS = [
  "raise_corner", "lower_corner", "raise_tiles", "lower_tiles", "flatten", "smooth",
  "raise_water", "lower_water", "carve", "fill",
] as const;

export type LandscapingTool = typeof LANDSCAPING_TOOLS[number];
export type TerrainCorner = "north_west" | "north_east" | "south_east" | "south_west";

export interface LandscapingSelection {
  readonly tool: LandscapingTool;
  readonly size: number;
  readonly height: number;
  readonly bottom: number;
  readonly top: number;
}

export interface TerrainEditCommand {
  readonly type: "terrain-edit";
  readonly command: LandscapingTool;
  readonly x: number;
  readonly y: number;
  readonly size: number;
  readonly corner: number;
  readonly steps: number;
  readonly height: number;
  readonly bottom: number;
  readonly top: number;
}

export interface TerrainEditResult {
  readonly type: "terrain-result";
  readonly success: boolean;
  readonly message: string;
  /** x, y, height triples for paths that the edit removed. */
  readonly invalidatedPaths: Int32Array;
}

export type LandscapingResult = TerrainEditResult;

export interface LandscapingTile {
  readonly x: number;
  readonly y: number;
  readonly height?: number;
  readonly corner?: TerrainCorner;
}

export type LandscapingPointerAction =
  | { readonly type: "hover" | "select" | "drag-start" | "drag-move" | "drag-end"; readonly tile: LandscapingTile | null }
  | { readonly type: "drag-cancel" };

export interface LandscapingPreview {
  readonly x: number;
  readonly y: number;
  readonly size: number;
  readonly tool: LandscapingTool;
  readonly height?: number;
  readonly corner?: TerrainCorner;
}

export interface LandscapingToolState {
  readonly active: boolean;
  readonly selection: LandscapingSelection | null;
  readonly preview: LandscapingPreview | null;
  readonly message: string;
}

const CORNER_INDEX: Record<TerrainCorner, number> = {
  north_west: 0,
  north_east: 1,
  south_east: 2,
  south_west: 3,
};

const TOOL_SET: ReadonlySet<string> = new Set(LANDSCAPING_TOOLS);
const CORNER_TOOLS: ReadonlySet<LandscapingTool> = new Set(["raise_corner", "lower_corner"]);
const INCREMENT_TOOLS: ReadonlySet<LandscapingTool> = new Set(["raise_corner", "lower_corner", "raise_tiles", "lower_tiles", "raise_water", "lower_water"]);

function validateSelection(selection: LandscapingSelection): void {
  if (!TOOL_SET.has(selection.tool) || !Number.isInteger(selection.size) || selection.size < 1 || selection.size > 9 ||
      !Number.isInteger(selection.height) || !Number.isInteger(selection.bottom) || !Number.isInteger(selection.top) ||
      selection.height < -32768 || selection.height > 32767 || selection.bottom < -32768 || selection.bottom > 32767 ||
      selection.top < -32768 || selection.top > 32767 || selection.bottom >= selection.top) {
    throw new RangeError("Invalid landscaping tool settings");
  }
}

function tileKey(tile: LandscapingTile): string {
  return `${tile.x},${tile.y}`;
}

function validateTile(tile: LandscapingTile): void {
  if (!Number.isInteger(tile.x) || !Number.isInteger(tile.y) || tile.x < 0 || tile.y < 0 ||
      (tile.height !== undefined && !Number.isInteger(tile.height))) {
    throw new RangeError("Invalid landscaping position");
  }
}

function tilesBetween(start: LandscapingTile, end: LandscapingTile): LandscapingTile[] {
  let x = start.x;
  let y = start.y;
  const deltaX = Math.abs(end.x - start.x);
  const deltaY = Math.abs(end.y - start.y);
  const stepX = start.x < end.x ? 1 : -1;
  const stepY = start.y < end.y ? 1 : -1;
  let error = deltaX - deltaY;
  const tiles: LandscapingTile[] = [];
  while (true) {
    tiles.push(x === end.x && y === end.y ? end : { x, y, height: end.height, corner: end.corner });
    if (x === end.x && y === end.y) return tiles;
    const doubledError = error * 2;
    if (doubledError > -deltaY) { error -= deltaY; x += stepX; }
    if (doubledError < deltaX) { error += deltaX; y += stepY; }
  }
}

function brushOrigin(tile: LandscapingTile, size: number, width: number, height: number): { x: number; y: number } {
  if (size > width || size > height) throw new RangeError("Landscaping brush is larger than the world");
  const offset = Math.floor((size - 1) / 2);
  return {
    x: Math.max(0, Math.min(width - size, tile.x - offset)),
    y: Math.max(0, Math.min(height - size, tile.y - offset)),
  };
}

export function createLandscapingTool(send: (command: TerrainEditCommand) => void, changed: () => void = () => {}) {
  let active = false;
  let selection: LandscapingSelection | null = null;
  let preview: LandscapingPreview | null = null;
  let message = "";
  let worldWidth = 65535;
  let worldHeight = 65535;
  let dragTiles = new Map<string, LandscapingTile>();
  let dragging = false;
  let lastDragTile: LandscapingTile | null = null;

  function validateWorldTile(tile: LandscapingTile): void {
    validateTile(tile);
    if (tile.x >= worldWidth || tile.y >= worldHeight) throw new RangeError("Landscaping position is outside the world");
  }

  function dimensions(width: number, height: number): void {
    if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width > 65535 || height > 65535) {
      throw new RangeError("Invalid landscaping world dimensions");
    }
    worldWidth = width;
    worldHeight = height;
  }

  function setPreview(tile: LandscapingTile | null): void {
    if (!tile || !selection) {
      preview = null;
      return;
    }
    validateWorldTile(tile);
    if (CORNER_TOOLS.has(selection.tool)) {
      preview = { x: tile.x, y: tile.y, size: 1, tool: selection.tool, height: tile.height, corner: tile.corner };
      return;
    }
    const origin = brushOrigin(tile, selection.size, worldWidth, worldHeight);
    preview = { ...origin, size: selection.size, tool: selection.tool, height: tile.height };
  }

  function commandAt(tile: LandscapingTile): void {
    if (!selection) throw new Error("Landscaping tool has no selection");
    validateWorldTile(tile);
    const cornerTool = CORNER_TOOLS.has(selection.tool);
    if (cornerTool && !tile.corner) {
      message = "Choose a terrain corner to sculpt.";
      return;
    }
    const origin = cornerTool ? { x: tile.x, y: tile.y } : brushOrigin(tile, selection.size, worldWidth, worldHeight);
    send({
      type: "terrain-edit",
      command: selection.tool,
      x: origin.x,
      y: origin.y,
      size: cornerTool ? 1 : selection.size,
      corner: tile.corner ? CORNER_INDEX[tile.corner] : 0,
      steps: INCREMENT_TOOLS.has(selection.tool) ? 1 : 0,
      height: selection.height,
      bottom: selection.bottom,
      top: selection.top,
    });
    message = "";
  }

  function addDragTile(tile: LandscapingTile | null): void {
    if (!tile) return;
    validateWorldTile(tile);
    dragTiles.set(tileKey(tile), tile);
    setPreview(tile);
  }

  return {
    state(): LandscapingToolState { return { active, selection, preview, message }; },
    setDimensions: dimensions,
    activate(next: LandscapingSelection): void {
      validateSelection(next);
      if (next.size > worldWidth || next.size > worldHeight) throw new RangeError("Landscaping brush is larger than the world");
      selection = { ...next };
      active = true;
      dragTiles.clear();
      dragging = false;
      lastDragTile = null;
      preview = null;
      message = "Drag over the map to apply the selected landscaping tool.";
      changed();
    },
    cancel(): void {
      active = false;
      selection = null;
      preview = null;
      dragTiles.clear();
      dragging = false;
      lastDragTile = null;
      message = "Landscaping tool cancelled.";
      changed();
    },
    handle(action: LandscapingPointerAction): boolean {
      if (!active || !selection) return false;
      if (action.type === "drag-cancel") {
        dragTiles.clear();
        dragging = false;
        lastDragTile = null;
        setPreview(null);
        changed();
        return true;
      }
      if (action.type === "hover") {
        if (!dragging) setPreview(action.tile);
        changed();
        return true;
      }
      if (action.type === "select") {
        if (action.tile) commandAt(action.tile);
        else message = "Choose a tile inside the park.";
        changed();
        return true;
      }
      if (action.type === "drag-start") {
        dragging = true;
        dragTiles.clear();
        addDragTile(action.tile);
        lastDragTile = action.tile;
        changed();
        return true;
      }
      if (action.type === "drag-move") {
        if (dragging && action.tile) {
          const segment = lastDragTile ? tilesBetween(lastDragTile, action.tile) : [action.tile];
          for (const tile of segment) addDragTile(tile);
          lastDragTile = action.tile;
        }
        changed();
        return true;
      }
      if (action.type === "drag-end") {
        if (!dragging) return true;
        if (action.tile) {
          const segment = lastDragTile ? tilesBetween(lastDragTile, action.tile) : [action.tile];
          for (const tile of segment) addDragTile(tile);
        }
        for (const tile of dragTiles.values()) commandAt(tile);
        dragTiles.clear();
        dragging = false;
        lastDragTile = null;
        changed();
        return true;
      }
      return true;
    },
    receive(result: TerrainEditResult): void {
      if (result.type !== "terrain-result" || !active) return;
      message = result.message;
      changed();
    },
  };
}

const GHOST_COLORS: Record<LandscapingTool, GeometryQuad["color"]> = {
  raise_corner: [0.86, 0.36, 0.24, 0.48],
  lower_corner: [0.24, 0.38, 0.70, 0.48],
  raise_tiles: [0.86, 0.36, 0.24, 0.36],
  lower_tiles: [0.24, 0.38, 0.70, 0.36],
  flatten: [0.91, 0.66, 0.18, 0.38],
  smooth: [0.14, 0.48, 0.43, 0.38],
  raise_water: [0.20, 0.49, 0.72, 0.42],
  lower_water: [0.20, 0.49, 0.72, 0.42],
  carve: [0.42, 0.32, 0.58, 0.42],
  fill: [0.64, 0.38, 0.26, 0.42],
};

function vertex(x: number, y: number, height: number, camera: Camera, viewport: Viewport): GeometryQuad["vertices"][number] {
  const projected = projectWorld(camera, viewport, { x, y, z: height / 4 });
  const rotated = [x + y, x - y, -x - y, -x + y][camera.rotation];
  return { ...projected, depth: rotated + height / 4, height, u: 0, v: 0 };
}

/** Builds a translucent brush footprint on the current ground, ready for a renderer geometry pass. */
export function buildLandscapingGhostQuads(
  preview: LandscapingPreview | null,
  world: WorldView,
  camera: Camera,
  viewport: Viewport,
): GeometryQuad[] {
  if (!preview) return [];
  const [width, height] = world.dimensions;
  if (!Number.isInteger(preview.x) || !Number.isInteger(preview.y) || !Number.isInteger(preview.size) ||
      preview.size < 1 || preview.x < 0 || preview.y < 0 || preview.x + preview.size > width || preview.y + preview.size > height ||
      world.surfaces.length !== width * height * 6) {
    throw new RangeError("Landscaping preview is outside the world");
  }
  const quads: GeometryQuad[] = [];
  const color = GHOST_COLORS[preview.tool];
  if (CORNER_TOOLS.has(preview.tool) && preview.corner) {
    const surfaceIndex = (preview.y * width + preview.x) * 6;
    const cornerIndex = CORNER_INDEX[preview.corner];
    const height = preview.height ?? world.surfaces[surfaceIndex + cornerIndex]!;
    const cornerPoints: Record<TerrainCorner, readonly [number, number]> = {
      north_west: [preview.x, preview.y],
      north_east: [preview.x + 1, preview.y],
      south_east: [preview.x + 1, preview.y + 1],
      south_west: [preview.x, preview.y + 1],
    };
    const [cornerX, cornerY] = cornerPoints[preview.corner];
    const radius = 0.14;
    const vertices = [
      vertex(cornerX, cornerY - radius, height, camera, viewport),
      vertex(cornerX + radius, cornerY, height, camera, viewport),
      vertex(cornerX, cornerY + radius, height, camera, viewport),
      vertex(cornerX - radius, cornerY, height, camera, viewport),
    ] as GeometryQuad["vertices"];
    return [{ vertices, color, kind: "landscaping-preview", id: `landscaping-corner-preview-${preview.x}-${preview.y}`, tileX: preview.x, tileY: preview.y }];
  }
  for (let y = preview.y; y < preview.y + preview.size; y++) {
    for (let x = preview.x; x < preview.x + preview.size; x++) {
      const index = (y * width + x) * 6;
      const corners: readonly number[] = preview.height === undefined
        ? [world.surfaces[index]!, world.surfaces[index + 1]!, world.surfaces[index + 2]!, world.surfaces[index + 3]!]
        : [preview.height, preview.height, preview.height, preview.height];
      const vertices = [
        vertex(x, y, corners[0], camera, viewport),
        vertex(x + 1, y, corners[1], camera, viewport),
        vertex(x + 1, y + 1, corners[2], camera, viewport),
        vertex(x, y + 1, corners[3], camera, viewport),
      ] as GeometryQuad["vertices"];
      quads.push({ vertices, color, kind: "landscaping-preview", id: `landscaping-preview-${x}-${y}`, tileX: x, tileY: y });
    }
  }
  return quads;
}
