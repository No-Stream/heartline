import { projectWorld, type Camera, type Rotation, type Viewport } from "../../src/camera";
import { createWorldRenderer, loadAtlas, type Atlas, type WorldView } from "../../src/render";
import { buildTrackQuads, interpolateTrainPoses, trackStyle, type TrackPaint } from "../../src/render/track/geometry";
import { buildRiderQuads, buildTrainQuads } from "../../src/render/track/trains";
import { parseTrainLayout } from "../../src/train-layout";

/** The fixture paints its coasters in its own atlas's theme inks: track in steel, supports in wood. */
function fixturePaint(atlas: Atlas): TrackPaint {
  const style = trackStyle(atlas);
  return { track: style.steel, supports: style.wood, trains: style.train, ink: style.ink, lineWeight: style.lineWeight };
}

/** The starter coaster's train layout, used by the fixture's rendered trains. */
const fixtureTrain = parseTrainLayout({ model: "hybrid", trackMaterial: "hybrid", carsPerTrain: 6, rowsPerCar: 2, seatsPerRow: 2, lengthMetres: 18 });

const initializeStarterParkWorker = (worker: Worker, frozen = false): void => {
  worker.postMessage({ type: "init", seed: 2026n, park: "demo", guestCount: 12, frozen, manualBlockAnalysis: false });
};

declare global {
  interface Window {
    renderTrackFixture(rotation: Rotation): Promise<{ picks: string[] }>;
    checkTrainMotion(): Promise<{ previous: number[]; current: number[]; interpolated: number[][] }>;
    starterTrackPoints(): Promise<{ x: number; y: number; z: number }[]>;
    checkCoasterTransfer(): Promise<{ rails: number; ties: number; supports: number; shared: boolean; revisionChanged: boolean; geometryEqual: boolean; refreshedSupports: boolean }>;
    renderRiderOrientations(): Promise<Record<"upright" | "banked" | "inverted", { pickable: boolean; hiddenByNearerGeometry: boolean; visibleOverFartherGeometry: boolean; topMinusBottom: { x: number; y: number } }>>;
  }
}

window.renderRiderOrientations = async () => {
  const { atlas, images } = await loadAtlas(new URL("/atlas.json", location.href).href);
  const canvas = document.createElement("canvas");
  canvas.width = 800; canvas.height = 500; document.body.append(canvas);
  const renderer = createWorldRenderer(canvas, atlas, images);
  const camera = { centerX: 4, centerY: 4, zoom: 2.5, rotation: 0 as const };
  const world: WorldView = { dimensions: new Uint16Array([8, 8]), surfaces: new Int32Array(8 * 8 * 6), paths: new Int32Array() };
  const diagonal = Math.SQRT1_2;
  const poses = new Float32Array([
    0, 2, 4, 1, 1, 0, 0, 0, 0, 1, 1,
    1, 4, 4, 1, 1, 0, 0, 0, diagonal, diagonal, 1,
    2, 6, 4, 1, 1, 0, 0, 0, 0, -1, 1,
  ]);
  const riders = buildRiderQuads(poses, camera, canvas, atlas, "3", fixtureTrain);
  const geometry = (view: Camera, viewport: Viewport, currentAtlas: Atlas, scale: string) => [
    ...buildTrainQuads(poses, view, viewport, fixturePaint(currentAtlas), fixtureTrain),
    ...buildRiderQuads(poses, view, viewport, currentAtlas, scale, fixtureTrain),
  ];
  renderer.render({ world: world, camera: camera, waterFrame: 0, geometryPasses: [geometry] });
  const pickRider = (rider: typeof riders[number]): boolean => {
    const minX = Math.floor(Math.min(...rider.vertices.map((vertex) => vertex.x)));
    const maxX = Math.ceil(Math.max(...rider.vertices.map((vertex) => vertex.x)));
    const minY = Math.floor(Math.min(...rider.vertices.map((vertex) => vertex.y)));
    const maxY = Math.ceil(Math.max(...rider.vertices.map((vertex) => vertex.y)));
    for (let y = minY; y <= maxY; y += 2) for (let x = minX; x <= maxX; x += 2) {
      const pick = renderer.pick(x, y);
      if (pick !== null && "id" in pick && pick.id === rider.id) return true;
    }
    return false;
  };
  const pickable = riders.map(pickRider);
  const upright = riders[0];
  const minX = Math.min(...upright.vertices.map((vertex) => vertex.x));
  const maxX = Math.max(...upright.vertices.map((vertex) => vertex.x));
  const minY = Math.min(...upright.vertices.map((vertex) => vertex.y));
  const maxY = Math.max(...upright.vertices.map((vertex) => vertex.y));
  const renderWithOccluder = (depth: number) => {
    const vertices = [[minX, minY], [maxX, minY], [maxX, maxY], [minX, maxY]].map(([x, y]) => ({ x, y, depth, height: 0, u: 0, v: 0 })) as typeof upright.vertices;
    renderer.render({ world: world, camera: camera, waterFrame: 0, geometryPasses: [geometry, () => [{ vertices, color: [0, 0, 0, 1] as const, kind: "support", id: "occluder", tileX: 0, tileY: 0 }]] });
  };
  renderWithOccluder(Math.max(...upright.vertices.map((vertex) => vertex.depth)) + 1);
  const hiddenByNearerGeometry = !pickRider(upright);
  renderWithOccluder(Math.min(...upright.vertices.map((vertex) => vertex.depth)) - 1);
  const visibleOverFartherGeometry = pickRider(upright);
  renderer.render({ world: world, camera: camera, waterFrame: 0, geometryPasses: [geometry] });
  return Object.fromEntries(riders.map((rider, index) => {
    const [topLeft, topRight, bottomRight, bottomLeft] = rider.vertices;
    const topMinusBottom = { x: (topLeft.x + topRight.x - bottomLeft.x - bottomRight.x) / 2, y: (topLeft.y + topRight.y - bottomLeft.y - bottomRight.y) / 2 };
    return [["upright", "banked", "inverted"][index], { pickable: pickable[index], hiddenByNearerGeometry, visibleOverFartherGeometry, topMinusBottom }];
  })) as Record<"upright" | "banked" | "inverted", { pickable: boolean; hiddenByNearerGeometry: boolean; visibleOverFartherGeometry: boolean; topMinusBottom: { x: number; y: number } }>;
};

window.checkCoasterTransfer = () => new Promise((resolve, reject) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  let first: { trackRevision: string; rails: Float32Array; ties: Float32Array; supports: Float32Array } | undefined;
  worker.onerror = (event) => { worker.terminate(); reject(new Error(event.message)); };
  worker.onmessage = (event: MessageEvent<{ type: string; coasters?: { id: number; trackRevision: string; rails?: Float32Array; ties?: Float32Array; supports?: Float32Array }[] }>) => {
    const message = event.data;
    if (message.type !== "world") return;
    const coaster = message.coasters?.[0];
    if (!coaster) { worker.terminate(); reject(new Error("Starter coaster geometry missing")); return; }
    if (!first) {
      if (!coaster.rails || !coaster.ties || !coaster.supports) { worker.terminate(); reject(new Error("Coaster geometry missing")); return; }
      first = { trackRevision: coaster.trackRevision, rails: coaster.rails, ties: coaster.ties, supports: coaster.supports };
      worker.postMessage({ type: "set-surface", x: 10, y: 20, corners: new Int16Array([1, 2, 3, 4]) });
    } else {
      worker.terminate();
      const rails = coaster.rails;
      const ties = coaster.ties;
      const geometryEqual = rails !== undefined && ties !== undefined
        && first.rails.length === rails.length && first.rails.every((value, index) => value === rails[index])
        && first.ties.length === ties.length && first.ties.every((value, index) => value === ties[index]);
      resolve({ rails: first.rails.length, ties: first.ties.length, supports: first.supports.length,
        shared: [first.rails, first.ties, first.supports].some((data) => !(data.buffer instanceof ArrayBuffer)),
        revisionChanged: first.trackRevision !== coaster.trackRevision, geometryEqual,
        refreshedSupports: coaster.supports instanceof Float32Array && coaster.supports !== first.supports });
    }
  };
  initializeStarterParkWorker(worker);
});

window.renderTrackFixture = async (rotation) => {
  const texture = document.createElement("canvas");
  texture.width = 48; texture.height = 16;
  const context = texture.getContext("2d");
  if (!context) throw new Error("Canvas 2D is unavailable");
  for (const [index, color] of ["#58ad56", "#d3b27a", "#795438"].entries()) {
    context.fillStyle = color; context.fillRect(index * 16, 0, 16, 16);
  }
  const image = new Image(); image.src = texture.toDataURL(); await image.decode();
  const frame = (x: number) => ({ width: 16, height: 16, anchor: { x: 8, y: 8 }, placements: { "1": { page: 0, x, y: 0, width: 16, height: 16 } } });
  const atlas: Atlas = {
    themeStyle: { steel: "#6e264a", wood: "#91593d", train: "#2e7075", ink: "#211519", lineWeight: 1 },
    pages: { "1": [{ file: "synthetic.png", width: 48, height: 16 }] },
    frames: {
      "ground/grass_light_0/any/static/0": frame(0),
      ...Object.fromEntries(["maya", "otis", "priya", "gus"].flatMap((person) => ["se", "sw", "nw", "ne"].map((view) => [`people/${person}/${view}/idle/0`, frame(16)]))),
      ...Object.fromEntries(Array.from({ length: 16 }, (_, index) => [`ground/path_${index.toString(2).padStart(4, "0")}/any/static/0`, frame(16)])),
      ...Object.fromEntries(["left", "right"].flatMap((side) => [0, 1, 2].flatMap((variant) => [
        [`ground/cliff_${side}_${variant}/any/static/0`, frame(32)],
        [`ground/cliff_step_${side}_${variant}/any/static/0`, frame(32)],
      ]))),
    },
  };
  const canvas = document.createElement("canvas"); canvas.width = 640; canvas.height = 480;
  document.body.append(canvas);
  const renderer = createWorldRenderer(canvas, atlas, { "1": [image] });
  const surfaces = new Int32Array(8 * 8 * 6);
  for (let y = 1; y <= 3; y++) for (let x = 5; x <= 6; x++) {
    surfaces.set([8, 8, 8, 8, 0, 0], (y * 8 + x) * 6);
  }
  const rail = (id: number, start: number[], end: number[], lateral: number[]) => [id, ...start, ...lateral, 0, 0, 1, ...end, ...lateral, 0, 0, 1];
  const world: WorldView & { coasterRails: Float32Array; coasterTies: Float32Array; coasterSupports: Float32Array; trainPoses: Float32Array } = {
    dimensions: new Uint16Array([8, 8]), surfaces,
    paths: new Int32Array([3, 3, 0, 0, 4, 3, 0, 0]),
    carved: new Int32Array([5, 2, 0, 6, 6, 2, 0, 6]),
    coasterRails: new Float32Array([
      ...rail(1, [1, 3.5, 1], [7, 3.5, 1], [0, 1, 0]),
      ...rail(2, [4, 1, 2], [4, 6, 2], [1, 0, 0]),
      ...rail(3, [4.5, 2.5, 0.75], [6.5, 2.5, 0.75], [0, 1, 0]),
    ]),
    coasterTies: new Float32Array(),
    coasterSupports: new Float32Array([1, 2, 3.5, 0, 2, 3.5, 1]),
    trainPoses: new Float32Array([1, 2.5, 3.5, 1, 1, 0, 0, 0, 0, 1, 3]),
  };
  const camera = { centerX: 4, centerY: 3.5, zoom: 2.5, rotation };
  renderer.render({ world: world, camera: camera, waterFrame: 0, geometryPasses: [(renderCamera, viewport, renderAtlas, scale) => [
    ...buildTrackQuads(world.coasterRails, world.coasterTies, world.coasterSupports, renderCamera, viewport, fixturePaint(renderAtlas)),
    ...buildTrainQuads(world.trainPoses, renderCamera, viewport, fixturePaint(renderAtlas), fixtureTrain),
    ...buildRiderQuads(world.trainPoses, renderCamera, viewport, renderAtlas, scale, fixtureTrain),
  ]] });
  const picks = new Set<string>();
  for (const point of [{ x: 3, y: 3.5, z: 1 }, { x: 4, y: 5, z: 2 }]) {
    const screen = projectWorld(camera, canvas, point);
    for (let dy = -8; dy <= 8; dy += 2) for (let dx = -8; dx <= 8; dx += 2) {
      const pick = renderer.pick(screen.x + dx, screen.y + dy);
      if (pick && "id" in pick) picks.add(`${pick.kind}:${pick.id}`);
    }
  }
  // Keep the projected crossing available for interactive inspection.
  canvas.dataset.crossing = JSON.stringify(projectWorld(camera, canvas, { x: 4, y: 3.5, z: 2 }));
  return { picks: [...picks].sort() };
};

window.checkTrainMotion = () => new Promise((resolve, reject) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  let previous: Float32Array | undefined;
  let coasterId: number | undefined;
  let workerReady = false;
  let testLapStarted = false;
  const startTestLap = (): void => {
    if (!workerReady || coasterId === undefined || testLapStarted) return;
    testLapStarted = true;
    worker.postMessage({ type: "ride-edit", ride: coasterId });
  };
  worker.onerror = (event) => { worker.terminate(); reject(new Error(event.message)); };
  worker.onmessage = (event: MessageEvent<{ type: string; action?: string; success?: boolean; coasters?: { id: number; trainPoses?: Float32Array }[] }>) => {
    if (event.data.type === "coasters") {
      coasterId ??= event.data.coasters?.[0]?.id;
      startTestLap();
      return;
    }
    if (event.data.type === "ready") {
      workerReady = true;
      startTestLap();
      return;
    }
    if (event.data.type === "coaster-action-result" && event.data.action === "edit" && event.data.success) {
      worker.postMessage({ type: "construction-test" });
      return;
    }
    if (event.data.type !== "snapshot" || coasterId === undefined) return;
    const current = event.data.coasters?.find(({ id }) => id === coasterId)?.trainPoses;
    if (!current?.length) return;
    if (!previous) { previous = current; return; }
    if (current[1] === previous[1] && current[2] === previous[2] && current[3] === previous[3]) return;
    const interpolated = [0, 0.25, 0.5, 0.75, 1].map((fraction) => [...interpolateTrainPoses(previous!, current, fraction).slice(0, 11)]);
    worker.terminate();
    resolve({ previous: [...previous.slice(0, 11)], current: [...current.slice(0, 11)], interpolated });
  };
  initializeStarterParkWorker(worker);
});

window.starterTrackPoints = () => new Promise((resolve, reject) => {
  const worker = new Worker(new URL("../../src/worker.ts", import.meta.url), { type: "module" });
  worker.onerror = (event) => { worker.terminate(); reject(new Error(event.message)); };
  worker.onmessage = (event: MessageEvent<{ type: string; coasters?: { id: number; rails?: Float32Array }[] }>) => {
    if (event.data.type !== "world") return;
    const geometry = event.data.coasters?.[0]?.rails;
    if (!geometry) return;
    const points: { x: number; y: number; z: number }[] = [];
    for (let index = 0; index < geometry.length; index += 19) {
      points.push({ x: geometry[index + 1], y: geometry[index + 2], z: geometry[index + 3] });
    }
    worker.terminate(); resolve(points);
  };
  initializeStarterParkWorker(worker);
});
