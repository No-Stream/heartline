import type { Rotation } from "../camera";
import type { Atlas, WorldSprite } from "./scene";

export interface GuestSnapshot {
  readonly guestIds: Uint32Array;
  readonly guests: Float32Array;
  readonly appearances: Uint8Array;
}

const RECORD_SIZE = 7;
const APPEARANCE_SIZE = 5;
const TILE_EDGE_UNITS = 32 / Math.SQRT1_2;
const VIEWS = ["se", "sw", "nw", "ne"] as const;
const spriteCache = new WeakMap<Atlas, string[]>();

function personSprites(atlas: Atlas): string[] {
  let sprites = spriteCache.get(atlas);
  if (!sprites) {
    sprites = [...new Set(Object.keys(atlas.frames).filter((key) => key.startsWith("people/")).map((key) => key.split("/").slice(0, 2).join("/")))].sort();
    if (!sprites.length) throw new Error("Atlas has no guest sprites");
    spriteCache.set(atlas, sprites);
  }
  return sprites;
}
const animationCache = new WeakMap<Atlas, Map<string, { frames: number; cycleLength: number }>>();

function animationInfo(atlas: Atlas, sprite: string, view: string, animation: string): { frames: number; cycleLength: number } {
  let cache = animationCache.get(atlas);
  if (!cache) { cache = new Map(); animationCache.set(atlas, cache); }
  const key = `${sprite}/${view}/${animation}`;
  const cached = cache.get(key);
  if (cached) return cached;
  let frames = 0;
  while (atlas.frames[`${key}/${frames}`]) frames++;
  const first = atlas.frames[`${key}/0`];
  const cycleLength = Number(first?.properties?.["walk-cycle-length"]);
  if (!frames || !Number.isFinite(cycleLength) || cycleLength <= 0) throw new Error(`Invalid guest animation ${key}`);
  const info = { frames, cycleLength };
  cache.set(key, info);
  return info;
}

function validate(snapshot: GuestSnapshot): void {
  if (snapshot.guests.length !== snapshot.guestIds.length * RECORD_SIZE || snapshot.appearances.length !== snapshot.guestIds.length * APPEARANCE_SIZE) throw new Error("Invalid guest snapshot");
}

export function buildPeople(previous: GuestSnapshot | undefined, current: GuestSnapshot, interpolation: number, rotation: Rotation, atlas: Atlas): WorldSprite[] {
  validate(current);
  if (previous) validate(previous);
  if (!Number.isFinite(interpolation) || interpolation < 0 || interpolation > 1) throw new RangeError("Invalid guest interpolation");
  const previousIndices = new Map<number, number>();
  previous?.guestIds.forEach((id, index) => previousIndices.set(id, index));
  const sprites: WorldSprite[] = [];
  const choices = personSprites(atlas);
  for (let index = 0; index < current.guestIds.length; index++) {
    const id = current.guestIds[index];
    const offset = index * RECORD_SIZE;
    const oldOffset = (previousIndices.get(id) ?? -1) * RECORD_SIZE;
    const value = (field: number): number => {
      const now = current.guests[offset + field];
      const before = oldOffset < 0 ? now : previous!.guests[oldOffset + field];
      return before + (now - before) * interpolation;
    };
    const appearanceOffset = index * APPEARANCE_SIZE;
    const [skin, hair, clothing, body, age] = current.appearances.subarray(appearanceOffset, appearanceOffset + APPEARANCE_SIZE);
    if (age > 2) throw new Error("Invalid guest age");
    const sprite = choices[(skin * 31 + hair * 17 + clothing * 7 + body + age * 13) % choices.length];
    const headingX = current.guests[offset + 3];
    const headingY = current.guests[offset + 4];
    const heading = Math.abs(headingX) >= Math.abs(headingY) ? (headingX >= 0 ? 0 : 2) : (headingY >= 0 ? 1 : 3);
    const view = VIEWS[(heading + rotation) % 4];
    const animation = current.guests[offset + 6] ? "walk" : "idle";
    const info = animationInfo(atlas, sprite, view, animation);
    const frame = animation === "walk" ? Math.floor(value(5) * TILE_EDGE_UNITS / info.cycleLength * info.frames) % info.frames : 0;
    sprites.push({ id: `guest-${id}`, sprite, x: value(0), y: value(1), height: value(2) * 4, view, animation, frame });
  }
  return sprites;
}
