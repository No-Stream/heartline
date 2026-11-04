import { expect, it } from "vitest";
import { blockLayoutChanged } from "./block-analysis-progress";
import type { BlockSection } from "./construction-protocol";

it("requests a redraw only when block section geometry or markers change", () => {
  const section: BlockSection = { boundaryMetres: 10, brakeStartMetres: 8, brakePieceIndices: [1], poweredHoldStartMetres: null, poweredHoldPieceIndices: [], editable: true, editableIndex: 0, reason: "brake", trackKind: "blockBrake" };
  expect(blockLayoutChanged([section], [{ ...section }])).toBe(false);
  expect(blockLayoutChanged([section], [{ ...section, boundaryMetres: 12 }])).toBe(true);
  expect(blockLayoutChanged([section], [])).toBe(true);
});
