import { expect, test } from "vitest";
import { parseTrainStates } from "./train-states";

test("parses one six-float train state record", () => {
  expect(parseTrainStates(new Float32Array([0, 2.5, 1, 0.4, -0.2, 1]))).toEqual([
    { trainIndex: 0, speed: 2.5, pieceKind: 1, verticalG: Math.fround(0.4), lateralG: Math.fround(-0.2), occupied: true },
  ]);
});

test("parses multiple records and accepts an empty snapshot", () => {
  expect(parseTrainStates(new Float32Array([
    0, 4, 0, 1.1, 0.2, 0,
    1, 12, 2, 2.4, -0.8, 1,
  ]))).toEqual([
    { trainIndex: 0, speed: 4, pieceKind: 0, verticalG: Math.fround(1.1), lateralG: Math.fround(0.2), occupied: false },
    { trainIndex: 1, speed: 12, pieceKind: 2, verticalG: Math.fround(2.4), lateralG: Math.fround(-0.8), occupied: true },
  ]);
  expect(parseTrainStates(new Float32Array())).toEqual([]);
});

test.each([
  ["partial records", [0, 1, 0, 1, 0]],
  ["negative train indices", [-1, 1, 0, 1, 0, 0]],
  ["fractional train indices", [0.5, 1, 0, 1, 0, 0]],
  ["negative speeds", [0, -1, 0, 1, 0, 0]],
  ["duplicate train indices", [0, 1, 0, 1, 0, 0, 0, 2, 0, 1, 0, 1]],
  ["unknown piece kinds", [0, 1, 5, 1, 0, 0]],
  ["fractional piece kinds", [0, 1, 1.5, 1, 0, 0]],
  ["non-binary occupancy", [0, 1, 0, 1, 0, 0.5]],
  ["non-finite values", [0, 1, 0, Number.NaN, 0, 0]],
])("rejects %s", (_description, values) => {
  expect(() => parseTrainStates(new Float32Array(values))).toThrow("Invalid train state snapshot");
});
