import type { SceneryReport as Scenery } from "./generated/SceneryReport";
import type { SceneryReportKind as SceneryKind } from "./generated/SceneryReportKind";
import { projectWorld, type Camera, type Viewport } from "./camera";
import type { GeometryQuad, WorldSprite, WorldView } from "./render/scene";

export const SCENERY_SETS = {
  Garden: ["tree_oval", "bush_flowering", "flower_bed", "bench", "lamp", "fountain", "planter"],
  Woodland: ["tree_cone", "tree_cone_ochre", "tree_cypress", "tree_cypress_short", "tree_oval_coral", "tree_tiers", "fence", "signpost", "bin", "balloon_cart"],
} as const;
export type { SceneryReport as Scenery } from "./generated/SceneryReport";
export type { SceneryReportKind as SceneryKind } from "./generated/SceneryReportKind";
export interface SceneryTile { readonly x: number; readonly y: number; readonly z: number; readonly quarter: number }
export type SceneryCommand =
  | { readonly type: "scenery-place"; readonly kind: SceneryKind; readonly x: number; readonly y: number; readonly rotation: number; readonly quarter: number }
  | { readonly type: "scenery-remove"; readonly id: string };
export type SceneryPreview = SceneryTile & { readonly kind: SceneryKind; readonly rotation: number };

const directional = new Set<SceneryKind>(["flower_bed", "bench", "lamp", "fence", "signpost", "balloon_cart"]);

export function scenerySprites(items: readonly Scenery[], cameraRotation: number): WorldSprite[] {
  return items.map((item) => ({
    id: `scenery-${item.id}`, sprite: `scenery/${item.kind}`,
    x: item.x + (item.kind === "fountain" ? 0.5 : [0.25, 0.75, 0.75, 0.25][item.quarter]),
    y: item.y + (item.kind === "fountain" ? 0.5 : [0.25, 0.25, 0.75, 0.75][item.quarter]), height: item.z,
    view: directional.has(item.kind) ? `rot${((item.rotation - cameraRotation + 4) % 4) * 90}` : "any",
    animation: item.kind === "fountain" ? "loop" : "static",
  }));
}

export function createSceneryTool(send: (command: SceneryCommand) => void, changed: () => void = () => {}) {
  let mode: "inactive" | "place" | "remove" = "inactive";
  let selection: { kind: SceneryKind; rotation: number } | null = null;
  let preview: SceneryPreview | null = null;
  let message = "";
  return {
    state() { return { mode, selection, preview, message }; },
    place(kind: SceneryKind, rotation: number) {
      if (!Object.values(SCENERY_SETS).some((set: readonly string[]) => set.includes(kind)) || !Number.isInteger(rotation) || rotation < 0 || rotation > 3) throw new RangeError("Invalid scenery selection");
      mode = "place"; selection = { kind, rotation }; preview = null; message = "Choose a tile for the scenery."; changed();
    },
    remove() { mode = "remove"; selection = null; preview = null; message = "Choose scenery to remove."; changed(); },
    cancel() { mode = "inactive"; selection = null; preview = null; message = "Scenery selection cancelled."; changed(); },
    hover(tile: SceneryTile | null) { preview = mode === "place" && selection && tile ? { ...tile, ...selection } : null; changed(); },
    select(tile: SceneryTile | null, id?: string) {
      if (mode === "place" && tile && selection) send({ type: "scenery-place", ...selection, x: tile.x, y: tile.y, quarter: tile.quarter });
      else if (mode === "remove" && id !== undefined) send({ type: "scenery-remove", id });
      else if (mode !== "inactive") message = mode === "place" ? "Choose a tile inside the park." : "Choose scenery to remove.";
      changed();
    },
    receive(text: string) { if (mode !== "inactive") { message = text; changed(); } },
  };
}

export function buildSceneryGhostQuads(preview: SceneryPreview | null, world: WorldView, camera: Camera, viewport: Viewport): GeometryQuad[] {
  if (!preview) return [];
  const [width, height] = world.dimensions;
  if (preview.x < 0 || preview.y < 0 || preview.x >= width || preview.y >= height) throw new RangeError("Scenery preview is outside the world");
  const left = preview.kind === "fountain" ? 0 : [0, 0.5, 0.5, 0][preview.quarter];
  const top = preview.kind === "fountain" ? 0 : [0, 0, 0.5, 0.5][preview.quarter];
  const size = preview.kind === "fountain" ? 1 : 0.5;
  const points = [[preview.x + left, preview.y + top], [preview.x + left + size, preview.y + top], [preview.x + left + size, preview.y + top + size], [preview.x + left, preview.y + top + size]] as const;
  const vertices = points.map(([x, y]) => {
    const elevation = preview.z;
    const projected = projectWorld(camera, viewport, { x, y, z: elevation / 4 });
    return { ...projected, depth: [x + y, x - y, -x - y, -x + y][camera.rotation] + elevation / 4, height: elevation, u: 0, v: 0 };
  }) as unknown as GeometryQuad["vertices"];
  return [{ vertices, color: [0.2, 0.6, 0.45, 0.45], kind: "landscaping-preview", id: "scenery-preview", tileX: preview.x, tileY: preview.y }];
}
