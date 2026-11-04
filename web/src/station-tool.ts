import type { Tool } from "./tool";

export interface StationTile { readonly x: number; readonly y: number; readonly height: number }
export interface Station { x: number; y: number; z: number; heading: number; locked: boolean }
type Point = { readonly x: number; readonly y: number };
export type StationToolAction = { readonly type: "hover" | "select"; readonly position: Point | null; readonly tile: StationTile | null; readonly preserve: boolean; readonly project: (point: { x: number; y: number; z: number }) => Point };
export type StationToolCommand = { readonly type: "construction-start"; readonly x: number; readonly y: number; readonly z: number; readonly heading: number };
export interface StationToolState { readonly active: boolean; readonly station: Station | undefined; readonly height: number }
const directions = [[1, 0], [0, 1], [-1, 0], [0, -1]] as const;

export function createStationTool(send: (command: StationToolCommand) => void, changed: () => void = () => {}): Tool<StationToolAction, StationToolState> & { activate(): void; setHeight(height: number): void; ghost(): Float32Array } {
  let active = false;
  let height = 12;
  let station: Station | undefined;
  function update(action: StationToolAction): void {
    if (!action.position) { if (!station?.locked) station = undefined; return; }
    if (station?.locked) {
      const origin = action.project({ x: station.x + 0.5, y: station.y + 0.5, z: (station.z - height) / 4 });
      const delta = { x: action.position.x - origin.x, y: action.position.y - origin.y };
      if (Math.hypot(delta.x, delta.y) > 4) {
        let best = -Infinity;
        directions.forEach(([dx, dy], index) => {
          const end = action.project({ x: station!.x + 0.5 + dx, y: station!.y + 0.5 + dy, z: (station!.z - height) / 4 });
          const vx = end.x - origin.x, vy = end.y - origin.y;
          const score = (delta.x * vx + delta.y * vy) / Math.hypot(vx, vy);
          if (score > best) { best = score; station!.heading = index; }
        });
      }
    } else if (action.tile) station = { x: action.tile.x, y: action.tile.y, z: Math.ceil(action.tile.height / 2) * 2 + height, heading: 0, locked: false };
    else if (!action.preserve) station = undefined;
  }
  return {
    state: () => ({ active, station, height }),
    activate() { active = true; station = undefined; changed(); },
    cancel() { active = false; station = undefined; changed(); },
    setHeight(next) { if (station) station.z += next - height; height = next; changed(); },
    handle(action) {
      if (!active) return false;
      update(action);
      if (action.type === "select" && station) {
        if (station.locked) { send({ type: "construction-start", x: station.x, y: station.y, z: station.z, heading: station.heading }); active = false; station = undefined; }
        else station.locked = true;
      }
      changed();
      return true;
    },
    ghost() {
      if (!active || !station) return new Float32Array();
      const [dx, dy] = directions[station.heading];
      const side = [-dy, dx, 0];
      const z = station.z / 4;
      return new Float32Array([-1, station.x - side[0] * 0.18, station.y - side[1] * 0.18, z, ...side, 0, 0, 1, station.x + dx * 4 - side[0] * 0.18, station.y + dy * 4 - side[1] * 0.18, z, ...side, 0, 0, 1,
        -1, station.x + side[0] * 0.18, station.y + side[1] * 0.18, z, ...side, 0, 0, 1, station.x + dx * 4 + side[0] * 0.18, station.y + dy * 4 + side[1] * 0.18, z, ...side, 0, 0, 1]);
    },
  };
}
