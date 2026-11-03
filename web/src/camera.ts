import type { CameraAction, ScreenPoint } from "./input/actions";

export type Rotation = 0 | 1 | 2 | 3;
export interface Camera {
  readonly centerX: number;
  readonly centerY: number;
  readonly zoom: number;
  readonly rotation: Rotation;
}
export interface Viewport { readonly width: number; readonly height: number }
export interface WorldPoint { readonly x: number; readonly y: number; readonly z: number }

function rotate(x: number, y: number, rotation: Rotation): [number, number] {
  switch (rotation) {
    case 0: return [x, y];
    case 1: return [-y, x];
    case 2: return [-x, -y];
    case 3: return [y, -x];
  }
}

export function projectWorld(camera: Camera, viewport: Viewport, point: WorldPoint): ScreenPoint {
  const [x, y] = rotate(point.x - camera.centerX, point.y - camera.centerY, camera.rotation);
  return {
    x: viewport.width / 2 + (x - y) * 32 * camera.zoom,
    y: viewport.height / 2 + ((x + y) * 16 - point.z * 32) * camera.zoom,
  };
}

export function unprojectGround(camera: Camera, viewport: Viewport, point: ScreenPoint, height = 0): WorldPoint {
  const difference = (point.x - viewport.width / 2) / (32 * camera.zoom);
  const sum = ((point.y - viewport.height / 2) / camera.zoom + height * 32) / 16;
  const [x, y] = rotate((sum + difference) / 2, (sum - difference) / 2, ((4 - camera.rotation) % 4) as Rotation);
  return { x: x + camera.centerX, y: y + camera.centerY, z: height };
}

export function applyCameraAction(camera: Camera, viewport: Viewport, action: CameraAction): Camera {
  switch (action.type) {
    case "rotate":
      if (!Number.isInteger(action.turns)) throw new Error("Rotation must be a whole number of quarter turns");
      return { ...camera, rotation: (((camera.rotation + action.turns) % 4 + 4) % 4) as Rotation };
    case "pan": {
      const center = { x: viewport.width / 2, y: viewport.height / 2 };
      const displaced = unprojectGround(camera, viewport, { x: center.x - action.dx, y: center.y - action.dy });
      return { ...camera, centerX: displaced.x, centerY: displaced.y };
    }
    case "zoom": {
      if (!Number.isFinite(action.factor) || action.factor <= 0) throw new Error("Zoom factor must be positive and finite");
      const before = unprojectGround(camera, viewport, action.anchor);
      const zoom = Math.max(0.5, Math.min(4, camera.zoom * action.factor));
      const zoomed = { ...camera, zoom };
      const after = unprojectGround(zoomed, viewport, action.anchor);
      return { ...zoomed, centerX: camera.centerX + before.x - after.x, centerY: camera.centerY + before.y - after.y };
    }
  }
}
