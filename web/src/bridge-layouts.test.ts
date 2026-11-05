import { readFileSync } from "node:fs";
import { expect, test } from "vitest";
import { BRIDGE_LAYOUTS } from "./bridge-layouts";

test("Rust and TypeScript describe the same ordered bridge records", () => {
  const rust = readFileSync("crates/sim-wasm/src/layouts.rs", "utf8");
  for (const [name, fields] of Object.entries(BRIDGE_LAYOUTS)) {
    const declaration = rust.match(new RegExp(`pub const ${name}: &\\[&str\\] = &\\[([^\\]]*)\\];`));
    expect(declaration, `missing Rust layout ${name}`).not.toBeNull();
    expect(Array.from(declaration![1].matchAll(/"([^"]+)"/g), (match) => match[1])).toEqual(fields);
  }
  expect([...rust.matchAll(/pub const ([A-Z_]+): &\[&str\]/g)].map((match) => match[1]).sort()).toEqual(Object.keys(BRIDGE_LAYOUTS).sort());
});

test("train state records use the documented snapshot field order", () => {
  expect(BRIDGE_LAYOUTS.TRAIN_STATE).toEqual(["train_index", "speed", "piece_kind", "vertical_g", "lateral_g", "occupied"]);
});
