export const GAME_SPEEDS = [1, 2, 4, 8, 16] as const;
export type GameSpeed = (typeof GAME_SPEEDS)[number];

export function isGameSpeed(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 16;
}
