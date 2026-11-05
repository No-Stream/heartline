import { projectWorld, unprojectGround, type Camera, type Rotation } from "../../src/camera";
import { createWorldRenderer, type Atlas, type WorldView } from "../../src/render";
import { buildTrackQuads } from "../../src/render/track/geometry";

interface CatalogPiece { name: string; rails: number[]; ties: number[] }
declare global {
  interface Window { renderPieceCatalog(pieces: CatalogPiece[], rotation: Rotation): Promise<string[]> }
}

window.renderPieceCatalog = async (pieces, rotation) => {
  const atlasImage = new Image();
  const atlasCanvas = document.createElement("canvas");
  atlasCanvas.width = 16; atlasCanvas.height = 16;
  const atlasContext = atlasCanvas.getContext("2d")!;
  atlasContext.fillStyle = "#86b967"; atlasContext.fillRect(0, 0, 16, 16);
  atlasImage.src = atlasCanvas.toDataURL(); await atlasImage.decode();
  const frame = { width: 16, height: 16, anchor: { x: 8, y: 8 }, placements: { "1": { page: 0, x: 0, y: 0, width: 16, height: 16 } } };
  const atlas: Atlas = {
    pages: { "1": [{ file: "synthetic.png", width: 16, height: 16 }] },
    frames: {
      "ground/grass_light_0/any/static/0": frame,
      ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
        [`ground/cliff_${side}_${variant}/any/static/0`, frame],
        [`ground/cliff_step_${side}_${variant}/any/static/0`, frame],
      ]))),
    },
  };
  const source = document.createElement("canvas"); source.width = 180; source.height = 126;
  const renderer = createWorldRenderer(source, atlas, { "1": [atlasImage] });
  const composite = document.createElement("canvas");
  composite.width = 180 * 8; composite.height = 148 * Math.ceil(pieces.length / 8);
  document.body.append(composite);
  const context = composite.getContext("2d")!;
  context.fillStyle = "#f1e9da"; context.fillRect(0, 0, composite.width, composite.height);
  context.font = "11px sans-serif"; context.fillStyle = "#211519";
  const world: WorldView = { dimensions: new Uint16Array([1, 1]), surfaces: new Int32Array(6), paths: new Int32Array() };
  const empty = new Float32Array();
  const style = { track: "#ff5d5d", supports: "#c98a4c", trains: "#7fe0b8", ink: "#2a1b42", lineWeight: 1 };
  const missing: string[] = [];
  for (const [pieceIndex, piece] of pieces.entries()) {
    const rails = new Float32Array(piece.rails);
    const ties = new Float32Array(piece.ties);
    const points: { x: number; y: number; z: number }[] = [];
    for (let i = 0; i < rails.length; i += 19) {
      for (const offset of [1, 10]) points.push({ x: rails[i + offset], y: rails[i + offset + 1], z: rails[i + offset + 2] });
    }
    const bounds = (axis: "x" | "y" | "z") => [Math.min(...points.map((point) => point[axis])), Math.max(...points.map((point) => point[axis]))];
    const [minX, maxX] = bounds("x"), [minY, maxY] = bounds("y"), [minZ] = bounds("z");
    const shift = [0.5 - (minX + maxX) / 2, 0.5 - (minY + maxY) / 2, 4 - minZ];
    for (let i = 0; i < rails.length; i += 19) for (const offset of [1, 10]) for (let axis = 0; axis < 3; axis++) rails[i + offset + axis] += shift[axis];
    for (let i = 0; i < ties.length; i += 13) for (const offset of [1, 4]) for (let axis = 0; axis < 3; axis++) ties[i + offset + axis] += shift[axis];
    const viewport = { width: source.width, height: source.height };
    const base: Camera = { centerX: 0.5, centerY: 0.5, zoom: 1, rotation };
    const projected = points.map((point) => projectWorld(base, viewport, { x: point.x + shift[0], y: point.y + shift[1], z: point.z + shift[2] }));
    const minScreenX = Math.min(...projected.map((point) => point.x)), maxScreenX = Math.max(...projected.map((point) => point.x));
    const minScreenY = Math.min(...projected.map((point) => point.y)), maxScreenY = Math.max(...projected.map((point) => point.y));
    const center = unprojectGround(base, viewport, { x: (minScreenX + maxScreenX) / 2, y: (minScreenY + maxScreenY) / 2 });
    const zoom = Math.min(2, 140 / Math.max(1, maxScreenX - minScreenX), 95 / Math.max(1, maxScreenY - minScreenY));
    const camera: Camera = { ...base, centerX: center.x, centerY: center.y, zoom };
    const quads = buildTrackQuads(rails, ties, empty, camera, viewport, style);
    renderer.render({ world: world, camera: camera, waterFrame: 0, geometryPasses: [() => quads] });
    const picked = quads.some((quad) => {
      const x = quad.vertices.reduce((sum, vertex) => sum + vertex.x, 0) / 4;
      const y = quad.vertices.reduce((sum, vertex) => sum + vertex.y, 0) / 4;
      for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) {
        const pick = renderer.pick(x + dx, y + dy);
        if (pick && "id" in pick && pick.id === "track-0") return true;
      }
      return false;
    });
    if (!picked) missing.push(piece.name);
    const x = pieceIndex % 8 * 180, y = Math.floor(pieceIndex / 8) * 148;
    context.drawImage(source, x, y);
    context.fillText(piece.name, x + 3, y + 141, 174);
  }
  return missing;
};
