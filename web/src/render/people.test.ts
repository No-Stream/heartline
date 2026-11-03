import { describe, expect, test } from "vitest";
import { buildPeople, type GuestSnapshot } from "./people";
import type { Atlas } from "./scene";

const frames: Atlas["frames"] = {};
for (const name of ["maya", "otis", "priya", "gus", "lena", "noor", "sam", "hank", "theo", "bea", "ines", "kai"]) {
  for (const view of ["se", "sw", "nw", "ne"]) {
    for (const animation of ["walk", "idle"]) {
      for (let frame = 0; frame < (animation === "walk" ? 8 : 2); frame++) {
        frames[`people/${name}/${view}/${animation}/${frame}`] = { width: 20, height: 40, anchor: { x: 10, y: 40 }, placements: {}, properties: { "walk-cycle-length": "32" } };
      }
    }
  }
}
const atlas: Atlas = { pages: {}, frames };
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
    expect(people[0]).toMatchObject({ sprite: "people/bea", view: "sw", animation: "idle", frame: 0 });
    expect(people[1].sprite).not.toBe(people[0].sprite);
  });

  test("rejects malformed snapshots", () => {
    expect(() => buildPeople(undefined, snapshot([1], [1], appearance), 1, 0, atlas)).toThrow();
  });
});
