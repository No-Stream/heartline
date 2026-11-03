import { expect, test } from "vitest";
import { constructionRiderPositions } from "./game-test";

test("finds rider poses when active train car IDs are not contiguous", () => {
  const poses = new Float32Array([12, 1, 2, 3, 0, 1, 0, 0, 0, 1, 0, 13, 4, 5, 6, 0, 1, 0, 0, 0, 1, 0]);
  expect(constructionRiderPositions(poses, new Uint32Array([13, 0, 42]))).toEqual([
    { car: 13, seat: 0, id: "guest-42", x: 4, y: 5, z: 6 },
  ]);
});
