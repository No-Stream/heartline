import { projectWorld, type Camera, type Viewport } from "./camera";
import type { GeometryQuad, Triangle, Vertex, WorldSprite } from "./render/scene";
import type { TrackStyle } from "./render/track/geometry";

export type FacilityKind = "entrance" | "food_stall" | "drink_stall" | "restroom" | "carousel" | "ferris_wheel" | "souvenir";
export interface Facility { id: number; kind: FacilityKind; x: number; y: number; z: number; rotation: number; price: number; ride_state: "idle" | "running" | "unloading"; ride_progress: number }
export type FacilityAction = { type: "facility-place"; kind: FacilityKind; x: number; y: number; rotation: number; height?: number };

/** Building artwork in the atlas. Carousels and ferris wheels are drawn as geometry instead. */
export const facilityBuildings: Partial<Record<FacilityKind, string>> = { entrance: "park_gate", food_stall: "burger_stall", drink_stall: "burger_stall", restroom: "restroom", souvenir: "souvenir_shop" };

/** Tiles covered along x and y. */
export function facilityFootprint(facility: Facility): { width: number; height: number } {
  if (facility.kind !== "entrance") return { width: 2, height: 2 };
  return facility.rotation % 2 ? { width: 4, height: 1 } : { width: 1, height: 4 };
}

export function facilitySprites(facilities: readonly Facility[], cameraRotation: number): WorldSprite[] {
  return facilities.flatMap((facility) => {
    const building = facilityBuildings[facility.kind];
    const { width, height } = facilityFootprint(facility);
    if (!building) return [];
    const view = `rot${((facility.rotation - cameraRotation + 4) % 4) * 90}`;
    const gate = { id: `facility-${facility.id}`, sprite: `buildings/${building}`, x: facility.x + width / 2, y: facility.y + height / 2, height: facility.z, view, heightDepth: true };
    if (facility.kind !== "entrance") return [gate];
    return [gate, { ...gate, sprite: "buildings/ticket_booth", x: facility.x + 0.5, y: facility.y + 0.5, scale: 0.5 }];
  });
}

type Point = [number, number, number];
/** The wheel's centre is 1.7 tiles above the ground; angle 0 is level with the hub and the rim has radius 0.9. */
export function ferrisWheelPoint(facility: Facility, angle: number, radius: number): Point {
  const x = facility.x + 1, y = facility.y + 1, z = facility.z / 4;
  return facility.rotation % 2 ? [x, y + Math.cos(angle) * radius, z + 1.7 + Math.sin(angle) * radius] : [x + Math.cos(angle) * radius, y, z + 1.7 + Math.sin(angle) * radius];
}
export function carouselPoint(facility: Facility, angle: number, height: number, radius: number): Point {
  return [facility.x + 1 + Math.cos(angle) * radius, facility.y + 1 + Math.sin(angle) * radius, facility.z / 4 + height];
}
/** How far a ride has turned, in radians, so lights can turn with it. */
export function rideAngle(facility: Facility): number {
  return facility.ride_progress * (facility.kind === "carousel" ? 8 : 2) * Math.PI;
}
function color(hex: string): Triangle["color"] {
  return [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, 1];
}

// Simple geometry keeps rides visible until dedicated ride artwork is available.
export function buildFacilityGeometry(facilities: readonly Facility[], camera: Camera, viewport: Viewport, style: TrackStyle): GeometryQuad[] {
  const quads: GeometryQuad[] = [];
  for (const facility of facilities) {
    if (facility.kind !== "carousel" && facility.kind !== "ferris_wheel") continue;
    const x = facility.x + 1, y = facility.y + 1, z = facility.z / 4;
    const turned = rideAngle(facility);
    const project = ([px, py, pz]: Point): Vertex => ({ ...projectWorld(camera, viewport, { x: px, y: py, z: pz }), depth: [px + py, px - py, -px - py, -px + py][camera.rotation] + pz, height: pz * 4, u: 0, v: 0 });
    const quad = (points: [Point, Point, Point, Point], fill: string) => quads.push({ vertices: points.map(project) as GeometryQuad["vertices"], color: color(fill), kind: "facility", id: `facility-${facility.id}`, tileX: facility.x, tileY: facility.y });
    if (facility.kind === "carousel") {
      const count = 12;
      for (let i = 0; i < count; i++) {
        const a = i * 2 * Math.PI / count + turned, b = (i + 1) * 2 * Math.PI / count + turned;
        const p = (angle: number, height: number, radius: number): Point => carouselPoint(facility, angle, height, radius);
        quad([p(a, 0.1, 1), p(b, 0.1, 1), p(b, 0.25, 1), p(a, 0.25, 1)], style.wood);
        quad([p(a, 1.5, 1), p(b, 1.5, 1), [x, y, z + 1.9], [x, y, z + 1.9]], i % 2 ? style.train : style.steel);
        quad([p(a, 0.25, 0.8), p(a + 0.05, 0.25, 0.8), p(a + 0.05, 1.5, 0.8), p(a, 1.5, 0.8)], style.ink);
      }
    } else {
      const wheelPoint = (angle: number, radius: number): Point => ferrisWheelPoint(facility, angle, radius);
      for (let i = 0; i < 16; i++) {
        const a = i * 2 * Math.PI / 16 + turned, b = (i + 1) * 2 * Math.PI / 16 + turned;
        quad([wheelPoint(a, 0.9), wheelPoint(b, 0.9), wheelPoint(b, 0.82), wheelPoint(a, 0.82)], style.steel);
        quad([wheelPoint(a, 0.82), wheelPoint(a + 0.04, 0.82), wheelPoint(a + 0.04, 0.05), wheelPoint(a, 0.05)], style.wood);
      }
      quad([wheelPoint(Math.PI, 0.8), wheelPoint(Math.PI, 0.65), wheelPoint(0, 0.08), wheelPoint(Math.PI, 0.08)].map(([px, py, pz], i) => [px, py, i < 2 ? z : pz]) as [Point, Point, Point, Point], style.wood);
      quad([wheelPoint(0, 0.65), wheelPoint(0, 0.8), wheelPoint(0, 0.08), wheelPoint(Math.PI, 0.08)].map(([px, py, pz], i) => [px, py, i < 2 ? z : pz]) as [Point, Point, Point, Point], style.wood);
    }
  }
  return quads;
}
