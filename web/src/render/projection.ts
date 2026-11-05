import { projectWorld, type Camera, type Viewport, type WorldPoint } from "../camera";
import type { Vertex } from "./scene";

/** Projects world coordinates (z in tile units) to a vertex (height in height steps). */
export function projectVertex(camera: Camera, viewport: Viewport, point: WorldPoint, depthBias = 0): Vertex {
  const { x, y, z } = point;
  return {
    ...projectWorld(camera, viewport, point),
    depth: [x + y, x - y, -x - y, -x + y][camera.rotation] + z + depthBias,
    height: z * 4,
    u: 0,
    v: 0,
  };
}
