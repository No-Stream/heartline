import { projectWorld, type Camera, type Viewport } from "./camera";
import type { GeometryQuad, Triangle, Vertex, WorldSprite } from "./render/scene";
import type { TrackStyle } from "./render/track/geometry";

export type FacilityKind = "entrance" | "food_stall" | "drink_stall" | "restroom" | "carousel" | "ferris_wheel" | "souvenir_shop";
export interface Facility { id: number; kind: FacilityKind; x: number; y: number; z: number; rotation: number; price: number; ride_state: "idle" | "running" | "unloading"; ride_progress: number }
export type FacilityAction = { type: "facility-place"; kind: FacilityKind; x: number; y: number; rotation: number };

export function facilitySprites(facilities: readonly Facility[], cameraRotation: number): WorldSprite[] {
  const buildings: Partial<Record<FacilityKind, string>> = { entrance: "park_gate", food_stall: "burger_stall", drink_stall: "burger_stall", restroom: "restroom", souvenir_shop: "souvenir_shop" };
  return facilities.flatMap((facility) => {
    const building = buildings[facility.kind];
    const width = facility.kind === "entrance" ? (facility.rotation % 2 ? 4 : 1) : 2;
    const height = facility.kind === "entrance" ? (facility.rotation % 2 ? 1 : 4) : 2;
    if (!building) return [];
    const view = `rot${((facility.rotation - cameraRotation + 4) % 4) * 90}`;
    const gate = { id: `facility-${facility.id}`, sprite: `buildings/${building}`, x: facility.x + width / 2, y: facility.y + height / 2, height: facility.z, view };
    if (facility.kind !== "entrance") return [gate];
    return [gate, { ...gate, sprite: "buildings/ticket_booth", x: facility.x + 0.5, y: facility.y + 0.5, scale: 0.5 }];
  });
}

type Point = [number, number, number];
function color(hex: string): Triangle["color"] {
  return [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, 1];
}

// Simple geometry keeps rides visible until dedicated ride artwork is available.
export function buildFacilityGeometry(facilities: readonly Facility[], camera: Camera, viewport: Viewport, style: TrackStyle): GeometryQuad[] {
  const quads: GeometryQuad[] = [];
  for (const facility of facilities) {
    if (facility.kind !== "carousel" && facility.kind !== "ferris_wheel") continue;
    const x = facility.x + 1, y = facility.y + 1, z = facility.z / 4;
    const project = ([px, py, pz]: Point): Vertex => ({ ...projectWorld(camera, viewport, { x: px, y: py, z: pz }), depth: [px + py, px - py, -px - py, -px + py][camera.rotation] + pz, height: pz * 4, u: 0, v: 0 });
    const quad = (points: [Point, Point, Point, Point], fill: string) => quads.push({ vertices: points.map(project) as GeometryQuad["vertices"], color: color(fill), kind: "facility", id: `facility-${facility.id}`, tileX: facility.x, tileY: facility.y });
    if (facility.kind === "carousel") {
      const count = 12;
      for (let i = 0; i < count; i++) {
        const a = i * 2 * Math.PI / count + facility.ride_progress * 8 * Math.PI, b = (i + 1) * 2 * Math.PI / count + facility.ride_progress * 8 * Math.PI;
        const p = (angle: number, height: number, radius: number): Point => [x + Math.cos(angle) * radius, y + Math.sin(angle) * radius, z + height];
        quad([p(a, 0.1, 1), p(b, 0.1, 1), p(b, 0.25, 1), p(a, 0.25, 1)], style.wood);
        quad([p(a, 1.5, 1), p(b, 1.5, 1), [x, y, z + 1.9], [x, y, z + 1.9]], i % 2 ? style.train : style.steel);
        quad([p(a, 0.25, 0.8), p(a + 0.05, 0.25, 0.8), p(a + 0.05, 1.5, 0.8), p(a, 1.5, 0.8)], style.ink);
      }
    } else {
      const wheelPoint = (angle: number, radius: number): Point => facility.rotation % 2 ? [x, y + Math.cos(angle) * radius, z + 1.7 + Math.sin(angle) * radius] : [x + Math.cos(angle) * radius, y, z + 1.7 + Math.sin(angle) * radius];
      for (let i = 0; i < 16; i++) {
        const a = i * 2 * Math.PI / 16 + facility.ride_progress * 2 * Math.PI, b = (i + 1) * 2 * Math.PI / 16 + facility.ride_progress * 2 * Math.PI;
        quad([wheelPoint(a, 0.9), wheelPoint(b, 0.9), wheelPoint(b, 0.82), wheelPoint(a, 0.82)], style.steel);
        quad([wheelPoint(a, 0.82), wheelPoint(a + 0.04, 0.82), wheelPoint(a + 0.04, 0.05), wheelPoint(a, 0.05)], style.wood);
      }
      quad([wheelPoint(Math.PI, 0.8), wheelPoint(Math.PI, 0.65), wheelPoint(0, 0.08), wheelPoint(Math.PI, 0.08)].map(([px, py, pz], i) => [px, py, i < 2 ? z : pz]) as [Point, Point, Point, Point], style.wood);
      quad([wheelPoint(0, 0.65), wheelPoint(0, 0.8), wheelPoint(0, 0.08), wheelPoint(Math.PI, 0.08)].map(([px, py, pz], i) => [px, py, i < 2 ? z : pz]) as [Point, Point, Point, Point], style.wood);
    }
  }
  return quads;
}
