import { BRIDGE_LAYOUTS } from "../bridge-layouts";
import type { Rotation } from "../camera";
import type { PersonAppearance } from "../../art/generators/people.ts";
import type { Atlas, WorldSprite } from "./scene";

export interface GuestSnapshot {
  readonly guestIds: Uint32Array;
  readonly guests: Float32Array;
  readonly appearances: Uint8Array;
}

const RECORD_SIZE = BRIDGE_LAYOUTS.GUEST.length;
const APPEARANCE_SIZE = BRIDGE_LAYOUTS.GUEST_APPEARANCE.length;
const TILE_EDGE_UNITS = 32 / Math.SQRT1_2;
const VIEWS = ["se", "sw", "nw", "ne"] as const;
const SKIN_GROUPS = ["deep", "medium", "light"] as const;
const HAIR_GROUPS = ["curly", "short", "tied", "covered", "bob", "bald"] as const;
const CLOTHING_GROUPS = ["red", "orange", "yellow", "green", "blue", "purple", "neutral"] as const;
const BODY_SHAPES = ["slender", "average", "broad"] as const;
const SKIN_KEY_COUNT = 8;
const HAIR_KEY_COUNT = 12;
const CLOTHING_KEY_COUNT = 16;
const BODY_KEY_COUNT = 5;
const PERSON_METADATA_ERROR = "Atlas has invalid guest appearance metadata";

interface PersonCandidate { sprite: string; appearance: PersonAppearance }
type PeopleAtlas = Atlas & { sprites?: Record<string, { appearance?: PersonAppearance }> };

const spriteCache = new WeakMap<Atlas, PersonCandidate[]>();

function personSprites(atlas: Atlas): PersonCandidate[] {
  let sprites = spriteCache.get(atlas);
  if (!sprites) {
    sprites = [...new Set(Object.keys(atlas.frames).filter((key) => key.startsWith("people/")).map((key) => key.split("/").slice(0, 2).join("/")))].sort().map((sprite) => {
      const appearance = (atlas as PeopleAtlas).sprites?.[sprite]?.appearance;
      if (!appearance) throw new Error(`${PERSON_METADATA_ERROR}: ${sprite}`);
      return { sprite, appearance };
    });
    if (!sprites.length) throw new Error("Atlas has no guest sprites");
    spriteCache.set(atlas, sprites);
  }
  return sprites;
}

function categoryForKey<T extends string>(key: number, keyCount: number, groups: readonly T[]): T {
  return groups[Math.floor(key * groups.length / keyCount)];
}

function guestAppearanceMatches(candidate: PersonAppearance, hair: string, clothing: string, body: string): number {
  return Number(candidate.hairGroup === hair) + Number(candidate.clothingPaletteGroup === clothing) + Number(candidate.bodyShape === body);
}

function choosePerson(candidates: readonly PersonCandidate[], guestId: number, appearance: Uint8Array): string {
  const [skinKey, hairKey, clothingKey, bodyKey, ageKey] = appearance;
  if (skinKey >= SKIN_KEY_COUNT || hairKey >= HAIR_KEY_COUNT || clothingKey >= CLOTHING_KEY_COUNT || bodyKey >= BODY_KEY_COUNT || ageKey > 2) {
    throw new Error("Invalid guest appearance");
  }
  const ageGroup = (["child", "adult", "senior"] as const)[ageKey];
  const ageCandidates = candidates.filter(({ appearance: person }) => person.ageGroup === ageGroup);
  const eligible = ageGroup === "senior" && ageCandidates.length === 0
    ? candidates.filter(({ appearance: person }) => person.ageGroup === "adult")
    : ageCandidates;
  if (!eligible.length) throw new Error(`Atlas has no ${ageGroup} guest sprites`);

  const skin = categoryForKey(skinKey, SKIN_KEY_COUNT, SKIN_GROUPS);
  const hair = categoryForKey(hairKey, HAIR_KEY_COUNT, HAIR_GROUPS);
  const clothing = categoryForKey(clothingKey, CLOTHING_KEY_COUNT, CLOTHING_GROUPS);
  const body = categoryForKey(bodyKey, BODY_KEY_COUNT, BODY_SHAPES);
  const ranked = eligible.map((candidate) => ({
    ...candidate,
    skinMatch: candidate.appearance.skinToneGroup === skin,
    otherMatches: guestAppearanceMatches(candidate.appearance, hair, clothing, body),
  }));
  const bestSkinMatch = ranked.some(({ skinMatch }) => skinMatch);
  const skinRanked = ranked.filter(({ skinMatch }) => skinMatch === bestSkinMatch);
  const bestOtherMatches = Math.max(...skinRanked.map(({ otherMatches }) => otherMatches));
  const equallyGood = skinRanked.filter(({ otherMatches }) => otherMatches === bestOtherMatches);
  const hash = Math.imul(guestId ^ (guestId >>> 16), 0x45d9f3b) >>> 0;
  return equallyGood[hash % equallyGood.length].sprite;
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

export function riderAppearances(snapshot: GuestSnapshot, riders: Uint32Array, atlas: Atlas): Map<string, { id: string; sprite: string }> {
  validate(snapshot);
  if (riders.length % BRIDGE_LAYOUTS.RIDER.length) throw new Error("Invalid rider snapshot");
  const indices = new Map<number, number>();
  snapshot.guestIds.forEach((id, index) => indices.set(id, index));
  const result = new Map<string, { id: string; sprite: string }>();
  const choices = personSprites(atlas);
  for (let offset = 0; offset < riders.length; offset += BRIDGE_LAYOUTS.RIDER.length) {
    const [car, seat, id] = riders.subarray(offset, offset + BRIDGE_LAYOUTS.RIDER.length);
    const index = indices.get(id);
    const key = `${car}-${seat}`;
    if (index === undefined || seat >= 4 || result.has(key)) throw new Error("Invalid rider snapshot");
    const appearanceOffset = index * APPEARANCE_SIZE;
    result.set(key, { id: `guest-${id}`, sprite: choosePerson(choices, id, snapshot.appearances.subarray(appearanceOffset, appearanceOffset + APPEARANCE_SIZE)) });
  }
  return result;
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
    const appearance = current.appearances.subarray(appearanceOffset, appearanceOffset + APPEARANCE_SIZE);
    const sprite = choosePerson(choices, id, appearance);
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
