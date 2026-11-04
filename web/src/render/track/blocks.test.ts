import { describe, expect, test } from "vitest";
import { projectWorld } from "../../camera";
import { appendGeometry, pickScene, type Scene } from "../scene";
import { buildBlockBoundaryQuads, nearestBlockBoundaryMetres, type BlockSectionBoundary } from "./blocks";
import { buildTrackQuads, type TrackPaint, type TrackStyle } from "./geometry";

const camera = { centerX: 2, centerY: 2, zoom: 1, rotation: 0 as const };
const viewport = { width: 400, height: 300 };
const style: TrackStyle = { steel: "#ff0000", wood: "#aa6600", train: "#00ff00", ink: "#221133", lineWeight: 1 };
const paint: TrackPaint = { track: "#ff0000", supports: "#aa6600", trains: "#00ff00", ink: "#221133", lineWeight: 1 };

function railSegment(startX: number, endX: number): number[] {
  const records: number[] = [];
  for (const sideY of [1.82, 2.18]) {
    records.push(1, startX, sideY, 1, 0, 1, 0, 0, 0, 1, endX, sideY, 1, 0, 1, 0, 0, 0, 1);
  }
  return records;
}

const rails = new Float32Array([...railSegment(1, 2), ...railSegment(2, 3)]);
const railDistances = new Float32Array([0, 6, 6, 16]);
const sections: readonly BlockSectionBoundary[] = [
  { boundaryMetres: 0, editable: false, editableIndex: null, trackKind: "station" },
  { boundaryMetres: 6, editable: true, editableIndex: 0, trackKind: "blockBrake" },
  { boundaryMetres: 16, editable: true, editableIndex: 1, trackKind: "blockBrake" },
];

describe("block-section rendering", () => {
  test("colors consecutive track sections differently while preserving rail ink", () => {
    const quads = buildTrackQuads(rails, new Float32Array(), new Float32Array(), new Float32Array(), camera, viewport, paint, [0, 6], railDistances);
    const fillColors = (segmentIndex: number) => quads
      .slice(segmentIndex * 24, (segmentIndex + 1) * 24)
      .filter((quad) => quad.color[0] !== 34 / 255 || quad.color[1] !== 17 / 255 || quad.color[2] !== 51 / 255)
      .map((quad) => quad.color);

    expect(new Set(fillColors(0).map((color) => color.join(","))).size).toBe(1);
    expect(new Set(fillColors(1).map((color) => color.join(","))).size).toBe(1);
    expect(fillColors(0)[0]).not.toEqual(fillColors(1)[0]);
  });

  test("places pickable editable markers at their metre boundary and leaves fixed markers unpickable", () => {
    const markers = buildBlockBoundaryQuads(rails, railDistances, sections, camera, viewport, style);
    expect(markers).toHaveLength(6);
    expect(markers.map(({ kind, id }) => [kind, id])).toEqual([
      ["block-boundary-fixed", "block-boundary-fixed-0"],
      ["block-boundary-fixed", "block-boundary-fixed-0"],
      ["block-boundary", "block-boundary-0"],
      ["block-boundary", "block-boundary-0"],
      ["block-boundary", "block-boundary-1"],
      ["block-boundary", "block-boundary-1"],
    ]);
    expect(new Set(markers.map(({ color }) => color.join(","))).size).toBe(4);
    expect(markers[0].color).toEqual([34 / 255, 17 / 255, 51 / 255, 1]);
    expect(markers[1].color).not.toEqual(markers[0].color);
    expect(Math.max(...markers[0].vertices.map(({ x }) => x))).toBeGreaterThan(Math.max(...markers[1].vertices.map(({ x }) => x)));

    const scene: Scene = { triangles: [] };
    appendGeometry(scene, markers);
    const boundaryPoint = projectWorld(camera, viewport, { x: 2, y: 2, z: 1.08 });
    expect(pickScene(scene, boundaryPoint.x, boundaryPoint.y)).toMatchObject({ kind: "block-boundary", id: "block-boundary-0" });
    const fixedPoint = projectWorld(camera, viewport, { x: 1, y: 2, z: 1.08 });
    expect(pickScene(scene, fixedPoint.x, fixedPoint.y)).toBeNull();
  });

  test("maps screen-space dragging to the closest ordered rail distance in metres", () => {
    const point = projectWorld(camera, viewport, { x: 2.5, y: 2, z: 1 });
    expect(nearestBlockBoundaryMetres(rails, railDistances, point, camera, viewport)).toBeCloseTo(11);
    expect(nearestBlockBoundaryMetres(new Float32Array(), new Float32Array(), point, camera, viewport)).toBeUndefined();
  });
});
