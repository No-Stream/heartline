import { describe, expect, test } from "vitest";
import { buildPeople, type GuestSnapshot } from "./people";
import type { Atlas } from "./scene";
import { CAST, personAppearance } from "../../art/generators/people.ts";

const frames: Atlas["frames"] = {};
for (const person of CAST) {
  const name = person.id;
  for (const view of ["se", "sw", "nw", "ne"]) {
    for (const animation of ["walk", "idle"]) {
      for (let frame = 0; frame < (animation === "walk" ? 8 : 2); frame++) {
        frames[`people/${name}/${view}/${animation}/${frame}`] = { width: 20, height: 40, anchor: { x: 10, y: 40 }, placements: {}, properties: { "walk-cycle-length": "32" } };
      }
    }
  }
}
const atlas: Atlas & { sprites: Record<string, { appearance: ReturnType<typeof personAppearance> }> } = {
  pages: {},
  frames,
  sprites: Object.fromEntries(CAST.map((person) => [`people/${person.id}`, { appearance: personAppearance(person) }])),
};
const ageGroupBySprite = new Map(CAST.map(({ id, ageGroup }) => [`people/${id}`, ageGroup]));
const snapshot = (ids: number[], records: number[], appearances: number[]): GuestSnapshot => ({ guestIds: new Uint32Array(ids), guests: new Float32Array(records), appearances: new Uint8Array(appearances) });
const appearance = [1, 2, 3, 4, 1];

describe("guest rendering", () => {
  test("interpolates position, height and walking distance by stable ID", () => {
    const previous = snapshot([9, 3], [10, 20, 1, 1, 0, 0, 1, 5, 5, 0, 1, 0, 0, 0], [...appearance, ...appearance]);
    const current = snapshot([3, 9], [6, 5, 0, 0, 1, 1, 1, 12, 24, 2, -1, 0, 1, 1], [...appearance, ...appearance]);
    const people = buildPeople(previous, current, 0.5, 0, atlas);
    expect(people[1]).toMatchObject({ id: "guest-9", x: 11, y: 22, height: 6, view: "nw", animation: "walk", frame: 5 });
    expect(people[0]).toMatchObject({ id: "guest-3", x: 5.5, view: "sw" });
  });

  test("uses idle animation, appearance and camera-relative direction", () => {
    const current = snapshot([1, 2], [1, 2, 0, 1, 0, 0, 0, 2, 3, 0, 1, 0, 0, 0], [0, 0, 0, 0, 0, 1, 2, 3, 4, 1]);
    const people = buildPeople(undefined, current, 1, 1, atlas);
    expect(people[0]).toMatchObject({ view: "sw", animation: "idle", frame: 0 });
    expect(ageGroupBySprite.get(people[0].sprite)).toBe("child");
    expect(people[1].sprite).not.toBe(people[0].sprite);
  });

  test("rejects malformed snapshots", () => {
    expect(() => buildPeople(undefined, snapshot([1], [1], appearance), 1, 0, atlas)).toThrow();
  });

  test("uses adult designs for seniors when the atlas has no senior design", () => {
    const isSenior = (key: string): boolean => ageGroupBySprite.get(key.split("/").slice(0, 2).join("/")) === "senior";
    const withoutSenior: typeof atlas = {
      pages: {},
      frames: Object.fromEntries(Object.entries(atlas.frames).filter(([key]) => !isSenior(key))),
      sprites: Object.fromEntries(Object.entries(atlas.sprites).filter(([sprite]) => !isSenior(sprite))),
    };
    const person = buildPeople(undefined, snapshot([30], [0, 0, 0, 1, 0, 0, 0], [0, 0, 0, 0, 2]), 1, 0, withoutSenior)[0];
    expect(ageGroupBySprite.get(person.sprite)).toBe("adult");
  });

  test("keeps generated children on child designs and spreads choices deterministically", () => {
    const count = 6000;
    const ids = Array.from({ length: count }, (_, index) => index + 1);
    const records = ids.flatMap((_, index) => [index, 0, 0, 1, 0, 0, 0]);
    let seed = 0x12345678;
    const randomByte = (max: number): number => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      return Math.floor(seed / 0x1_0000_0000 * max);
    };
    const appearances = ids.flatMap(() => [randomByte(8), randomByte(12), randomByte(16), randomByte(5), randomByte(10) === 0 ? 2 : randomByte(4) === 0 ? 0 : 1]);
    const generated = snapshot(ids, records, appearances);
    const first = buildPeople(undefined, generated, 1, 0, atlas);
    const replay = buildPeople(undefined, generated, 1, 0, atlas);
    expect(first.map(({ sprite }) => sprite)).toEqual(replay.map(({ sprite }) => sprite));

    for (let index = 0; index < count; index++) {
      const age = appearances[index * 5 + 4];
      if (age === 0) expect(ageGroupBySprite.get(first[index].sprite)).toBe("child");
      if (age === 1) expect(ageGroupBySprite.get(first[index].sprite)).toBe("adult");
      if (age === 2) expect(ageGroupBySprite.get(first[index].sprite)).toBe("senior");
    }

    const counts = new Map<string, number>();
    for (const { sprite } of first) counts.set(sprite, (counts.get(sprite) ?? 0) + 1);
    const usedCounts = [...counts.values()].filter((value) => value > 0);
    expect(usedCounts.length).toBe(CAST.length);
    // Skin tone is matched before other traits, so spread is only comparable within an age group.
    for (const ageGroup of ["child", "adult", "senior"] as const) {
      const groupCounts = CAST.filter((person) => person.ageGroup === ageGroup).map(({ id }) => counts.get(`people/${id}`) ?? 0);
      expect(Math.max(...groupCounts), ageGroup).toBeLessThan(Math.min(...groupCounts) * 10);
    }
  });
});
