import { expect, it } from "vitest";
import { screamSlot } from "./scream-pool";

it("uses free scream groups before replacing the oldest active group", () => {
  const groups = [{ start: 0, end: 2 }, { start: 0.2, end: 1.2 }, null];
  expect(screamSlot(groups, 0.3)).toBe(2);
  expect(screamSlot([...groups.slice(0, 2), { start: 0.3, end: 1.5 }], 0.4)).toBe(0);
  expect(screamSlot([...groups.slice(0, 2), { start: 0.3, end: 1.5 }], 1.3)).toBe(1);
});
