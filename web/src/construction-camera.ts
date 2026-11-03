import { applyCameraAction, projectWorld, type Camera, type Viewport, type WorldPoint } from "./camera";

const railRecordLength = 19;
/** The fraction of the viewport on each side that the open track end may enter before the camera moves. */
const comfortMargin = 0.2;

/** Start and end points of each segment in the worker's rail geometry, in world tile units. */
export function railPoints(rails: Float32Array): WorldPoint[] {
  if (rails.length % railRecordLength !== 0) throw new Error("Rail geometry has an unexpected layout");
  const points: WorldPoint[] = [];
  for (let offset = 0; offset < rails.length; offset += railRecordLength) {
    points.push({ x: rails[offset + 1], y: rails[offset + 2], z: rails[offset + 3] }, { x: rails[offset + 10], y: rails[offset + 11], z: rails[offset + 12] });
  }
  return points;
}

function shift(minimum: number, maximum: number, size: number): number {
  const low = size * comfortMargin, high = size * (1 - comfortMargin);
  if (maximum - minimum > high - low) return (low + high) / 2 - (minimum + maximum) / 2;
  if (minimum < low) return low - minimum;
  if (maximum > high) return high - maximum;
  return 0;
}

/**
 * Move the camera a fraction of the way towards the smallest pan that brings the points inside the comfortable
 * middle of the view. It is settled once no pan is needed, so the player's own panning is never pulled back.
 */
export function followTrackEnd(camera: Camera, viewport: Viewport, points: readonly WorldPoint[], fraction: number): { camera: Camera; settled: boolean } {
  const screen = points.map((point) => projectWorld(camera, viewport, point));
  const dx = shift(Math.min(...screen.map(({ x }) => x)), Math.max(...screen.map(({ x }) => x)), viewport.width);
  const dy = shift(Math.min(...screen.map(({ y }) => y)), Math.max(...screen.map(({ y }) => y)), viewport.height);
  if (Math.hypot(dx, dy) < 0.5) return { camera, settled: true };
  return { camera: applyCameraAction(camera, viewport, { type: "pan", dx: dx * fraction, dy: dy * fraction }), settled: false };
}
