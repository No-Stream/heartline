import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { PieceShape } from "./construction-protocol";
import { initSync, piece_shapes_json } from "./wasm/theme_park_sim_wasm.js";
import { iconViews, pieceIconSvg } from "./piece-icons";

const wasmPath = fileURLToPath(new URL("./wasm/theme_park_sim_wasm_bg.wasm", import.meta.url));
if (!existsSync(wasmPath)) throw new Error(`Missing ${wasmPath}; run make build before running piece icon tests.`);
initSync(readFileSync(wasmPath));
const shapes = JSON.parse(piece_shapes_json()) as PieceShape[];

describe("piece icons", () => {
  it("draws finite, non-empty SVG paths for every catalog shape in the UI report", () => {
    expect(shapes.length).toBeGreaterThan(0);
    for (const shape of shapes) {
      for (const view of iconViews(shape)) {
        const svg = pieceIconSvg(shape, view, 46, 34);
        const pathData = [...svg.matchAll(/<path\b[^>]*\bd="([^"]*)"/g)].map(([, data]) => data);
        expect(pathData.length, shape.name).toBeGreaterThan(0);
        for (const data of pathData) {
          expect(data.trim().length, shape.name).toBeGreaterThan(0);
          expect(data, `${shape.name} (${view}): ${svg}`).not.toMatch(/NaN|Infinity/);
          const numbers = data.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? [];
          expect(numbers.length, shape.name).toBeGreaterThan(0);
          expect(numbers.every(Number.isFinite), shape.name).toBe(true);
        }
      }
    }
  });

  it("fails with the unknown bank name", () => {
    const bankedShape = shapes.find((shape) => shape.entryBank !== "Level" || shape.exitBank !== "Level");
    expect(bankedShape).toBeDefined();
    expect(() => pieceIconSvg({ ...bankedShape!, exitBank: "Diagonal" }, "twist", 46, 34)).toThrow("Unknown track bank: Diagonal");
  });
});
