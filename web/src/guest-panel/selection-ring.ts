import { projectVertex as projectWorldVertex } from "../render/projection";
import type { Camera, Viewport } from "../camera";
import type { Atlas, GeometryQuad, Vertex, WorldSprite } from "../render/scene";

const SEGMENTS = 24;
const RADIUS = 0.34;
const OPACITY = 0.84;

function inkColor(atlas: Atlas): GeometryQuad["color"] {
  const ink = atlas.themeStyle?.ink;
  if (!ink || !/^#[\da-f]{6}$/i.test(ink)) throw new Error("Atlas is missing selection ink style");
  const channels = [1, 3, 5].map((offset) => Number.parseInt(ink.slice(offset, offset + 2), 16) / 255);
  return [channels[0], channels[1], channels[2], OPACITY];
}

function projectVertex(
  guest: WorldSprite,
  camera: Camera,
  viewport: Viewport,
  x: number,
  y: number,
): Vertex {
  const z = guest.height / 4 + 0.015;
  return { ...projectWorldVertex(camera, viewport, { x, y, z }), height: guest.height };
}

/** Builds a theme-ink ring that stays on the selected guest's ground position. */
export function buildSelectionRing(
  guest: WorldSprite | undefined,
  camera: Camera,
  viewport: Viewport,
  atlas: Atlas,
): GeometryQuad[] {
  if (!guest) return [];
  if (![guest.x, guest.y, guest.height].every(Number.isFinite)) throw new Error("Invalid selected guest position");
  const color = inkColor(atlas);
  const thickness = 0.025 * atlas.themeStyle!.lineWeight;
  if (!Number.isFinite(thickness) || thickness <= 0) throw new Error("Invalid selection line weight");
  const quads: GeometryQuad[] = [];
  for (let index = 0; index < SEGMENTS; index += 1) {
    const firstAngle = (index / SEGMENTS) * Math.PI * 2;
    const secondAngle = ((index + 1) / SEGMENTS) * Math.PI * 2;
    const firstCosine = Math.cos(firstAngle);
    const firstSine = Math.sin(firstAngle);
    const secondCosine = Math.cos(secondAngle);
    const secondSine = Math.sin(secondAngle);
    const vertices = [
      projectVertex(guest, camera, viewport, guest.x + firstCosine * (RADIUS - thickness), guest.y + firstSine * (RADIUS - thickness)),
      projectVertex(guest, camera, viewport, guest.x + secondCosine * (RADIUS - thickness), guest.y + secondSine * (RADIUS - thickness)),
      projectVertex(guest, camera, viewport, guest.x + secondCosine * RADIUS, guest.y + secondSine * RADIUS),
      projectVertex(guest, camera, viewport, guest.x + firstCosine * RADIUS, guest.y + firstSine * RADIUS),
    ] as [Vertex, Vertex, Vertex, Vertex];
    quads.push({
      vertices,
      color,
      kind: "selection-ring",
      id: `selection-${guest.id}`,
      tileX: Math.floor(guest.x),
      tileY: Math.floor(guest.y),
    });
  }
  return quads;
}
