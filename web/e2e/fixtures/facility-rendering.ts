import { projectWorld, type Rotation } from "../../src/camera";
import { buildFacilityGeometry, facilityFootprint, facilitySprites, type Facility, type FacilityKind } from "../../src/facilities";
import { createWorldRenderer, loadAtlas, type Atlas, type WorldView } from "../../src/render";
import { trackStyle } from "../../src/render/track/geometry";

interface FacilityRenderResult {
  targetSamples: number;
  upperGroundSamples: number;
  upperReferenceSamples: number;
  bounds: { left: number; top: number; right: number; bottom: number };
}

declare global {
  interface Window {
    renderFacilityFixture(kind: FacilityKind, rotation: Rotation): Promise<FacilityRenderResult>;
  }
}

const facilityKinds: readonly FacilityKind[] = ["entrance", "food_stall", "drink_stall", "restroom", "souvenir", "carousel", "ferris_wheel"];

function facility(kind: FacilityKind, id: number, x: number, y: number): Facility {
  return { id, kind, x, y, z: 8, rotation: 0, price: 0, ride_state: "idle", ride_progress: 0 };
}

function buildWorld(facilities: readonly Facility[], includeContext: boolean, cameraRotation: Rotation, groundHeight = 8, raisedTerrain = true): WorldView {
  const width = 32;
  const height = 32;
  const target = facilities[0];
  const footprint = facilityFootprint(target);
  const surfaces = new Int32Array(width * height * 6);
  const paths = includeContext ? new Int32Array([target.x + footprint.width - 1, target.y + footprint.height, 8, 0]) : new Int32Array();
  const pathMetadata = includeContext ? new Int32Array([0, 0, 15]) : new Int32Array();

  for (let tileY = 0; tileY < height; tileY++) {
    for (let tileX = 0; tileX < width; tileX++) {
      const index = (tileY * width + tileX) * 6;
      surfaces.set([groundHeight, groundHeight, groundHeight, groundHeight, 0, 0], index);
    }
  }

  if (includeContext && raisedTerrain) {
    const raisedTerrainX = target.x - 1;
    const raisedTerrainY = target.y + footprint.height - 1;
    const raisedTerrainIndex = (raisedTerrainY * width + raisedTerrainX) * 6;
    surfaces.set([8, 12, 12, 8, 0, 0], raisedTerrainIndex);
  }

  return { dimensions: new Uint16Array([width, height]), surfaces, paths, pathMetadata, sprites: facilitySprites(facilities, cameraRotation) };
}

function screenBounds(kind: FacilityKind, target: Facility, rotation: Rotation, atlas: Atlas, canvas: HTMLCanvasElement): FacilityRenderResult["bounds"] {
  const camera = { centerX: 13, centerY: 13, zoom: 1, rotation };
  if (kind === "carousel" || kind === "ferris_wheel") {
    const targetQuads = buildFacilityGeometry([target], camera, canvas, trackStyle(atlas));
    const vertices = targetQuads.flatMap(({ vertices }) => vertices);
    return {
      left: Math.min(...vertices.map(({ x }) => x)) - 12,
      top: Math.min(...vertices.map(({ y }) => y)) - 12,
      right: Math.max(...vertices.map(({ x }) => x)) + 12,
      bottom: Math.max(...vertices.map(({ y }) => y)) + 12,
    };
  }

  const sprite = facilitySprites([target], rotation)[0];
  if (!sprite) throw new Error(`Facility ${kind} has no building sprite`);
  const view = sprite.view ?? "any";
  const frame = atlas.frames[`${sprite.sprite}/${view}/static/0`];
  if (!frame) throw new Error(`Missing atlas frame for ${sprite.sprite} ${view}`);
  const footprint = facilityFootprint(target);
  const anchor = projectWorld(camera, canvas, { x: target.x + footprint.width / 2, y: target.y + footprint.height / 2, z: target.z / 4 });
  return {
    left: anchor.x - frame.anchor.x,
    top: anchor.y - frame.anchor.y,
    right: anchor.x - frame.anchor.x + frame.width,
    bottom: anchor.y - frame.anchor.y + frame.height,
  };
}

function countTargetSamples(
  renderer: ReturnType<typeof createWorldRenderer>,
  id: string,
  bounds: FacilityRenderResult["bounds"],
  upperOnly: boolean,
): number {
  let count = 0;
  const top = Math.max(0, Math.floor(bounds.top));
  const bottom = Math.min(600, Math.ceil(upperOnly ? bounds.top + (bounds.bottom - bounds.top) * 0.55 : bounds.bottom));
  const left = Math.max(0, Math.floor(bounds.left));
  const right = Math.min(800, Math.ceil(bounds.right));
  for (let y = top; y < bottom; y += 4) {
    for (let x = left; x < right; x += 4) {
      const pick = renderer.pick(x, y);
      if (pick && "id" in pick && pick.id === id) count++;
    }
  }
  return count;
}

window.renderFacilityFixture = async (kind, rotation) => {
  if (!facilityKinds.includes(kind)) throw new Error(`Unsupported facility kind: ${kind}`);
  const { atlas, images } = await loadAtlas(new URL("/atlas.json", location.href).href);
  const canvas = document.createElement("canvas");
  canvas.width = 800;
  canvas.height = 600;
  document.body.replaceChildren(canvas);
  const renderer = createWorldRenderer(canvas, atlas, images, true);
  const target = facility(kind, 1, 12, 12);
  const footprint = facilityFootprint(target);
  const frontNeighbor = facility("food_stall", 2, 12, 10);
  const sideNeighbor = facility("restroom", 3, 12 + footprint.width, 12 + Math.max(0, Math.floor((footprint.height - 2) / 2)));
  const facilities = [target, frontNeighbor, sideNeighbor];
  const bounds = screenBounds(kind, target, rotation, atlas, canvas);
  const camera = { centerX: 13, centerY: 13, zoom: 1, rotation };
  const geometry = (view: typeof camera, viewport: { width: number; height: number }, currentAtlas: Atlas) =>
    buildFacilityGeometry(facilities, view, viewport, trackStyle(currentAtlas));

  renderer.render(buildWorld(facilities, true, rotation), camera, 0, [geometry]);
  renderer.waitForGpu();
  const targetSamples = countTargetSamples(renderer, "facility-1", bounds, false);
  const canvasContext = canvas.getContext("webgl2");
  if (!canvasContext) throw new Error("WebGL2 is unavailable");
  canvasContext.finish();
  const png = await new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Could not capture facility rendering fixture"))));
  const imageUrl = URL.createObjectURL(png);
  const image = new Image();
  image.src = imageUrl;
  await image.decode();
  const preview = document.createElement("canvas");
  preview.width = canvas.width;
  preview.height = canvas.height;
  const previewContext = preview.getContext("2d");
  if (!previewContext) throw new Error("Canvas 2D is unavailable");
  previewContext.drawImage(image, 0, 0);
  URL.revokeObjectURL(imageUrl);

  renderer.render(buildWorld([target], true, rotation, 8, false), camera, 0, [
    (view, viewport, currentAtlas) => buildFacilityGeometry([target], view, viewport, trackStyle(currentAtlas)),
  ]);
  renderer.waitForGpu();
  const upperGroundSamples = countTargetSamples(renderer, "facility-1", bounds, true);
  renderer.render(buildWorld([target], false, rotation, -100), camera, 0, [
    (view, viewport, currentAtlas) => buildFacilityGeometry([target], view, viewport, trackStyle(currentAtlas)),
  ]);
  renderer.waitForGpu();
  const upperReferenceSamples = countTargetSamples(renderer, "facility-1", bounds, true);
  canvas.replaceWith(preview);
  return { targetSamples, upperGroundSamples, upperReferenceSamples, bounds };
};
