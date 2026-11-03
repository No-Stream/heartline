// The art proof demo: an isometric park drawn with WebGL 2 from the sprite atlas, and a gallery.
// `?theme=<name>` shows one of the alternative themes built by `make art-themes`.
import { COLORS } from "./generators/palette.ts";
import { themeNamed } from "./generators/themes.ts";

interface Placement { page: number; x: number; y: number; width: number; height: number }
interface Frame {
  source: string;
  width: number;
  height: number;
  anchor: { x: number; y: number };
  properties: Record<string, string>;
  placements: Record<string, Placement>;
}
interface Atlas {
  scales: number[];
  pages: Record<string, { file: string; width: number; height: number }[]>;
  sprites: Record<string, { views: Record<string, Record<string, string[]>> }>;
  frames: Record<string, Frame>;
}

const TILE_HALF_WIDTH = 32;
const TILE_HALF_HEIGHT = 16;
/** World length of a tile edge in the units of data-walk-cycle-length. */
const TILE_EDGE_UNITS = 32 / Math.SQRT1_2;
const ISLAND_WIDTH = 24;
const ISLAND_DEPTH = 20;
const ROTATION_NAMES = ["rot0", "rot90", "rot180", "rot270"];
/** Headings +x, +y, -x, -y seen by an unrotated camera. */
const FACING_NAMES = ["se", "sw", "nw", "ne"];
const HEADING_STEPS: readonly [number, number][] = [[1, 0], [0, 1], [-1, 0], [0, -1]];

function required<T>(value: T | null | undefined, what: string): T {
  if (value === null || value === undefined) throw new Error(`Missing ${what}`);
  return value;
}
const element = <T extends HTMLElement>(id: string): T => required(document.getElementById(id) as T | null, `element #${id}`);

const theme = themeNamed(new URLSearchParams(location.search).get("theme") ?? "candy");
const ATLAS_DIRECTORY = theme.name === "candy" ? "./atlas" : `./themes/${theme.name}/atlas`;
document.documentElement.style.setProperty("--paper", theme.scene.background);
document.documentElement.style.setProperty("--stage", theme.color(COLORS.grass));
document.documentElement.style.setProperty("--stage-sides", theme.color(COLORS.grassAlternate));
document.title = `Park Art Proof: ${theme.name}`;
const atlas = (await (await fetch(`${ATLAS_DIRECTORY}/atlas.json`)).json()) as Atlas;
const pageImages: Record<string, HTMLImageElement[]> = {};
await Promise.all(
  atlas.scales.map(async (scale) => {
    pageImages[scale] = await Promise.all(
      atlas.pages[scale].map(async (page) => {
        const image = new Image();
        image.src = `${ATLAS_DIRECTORY}/${page.file}`;
        await image.decode();
        return image;
      }),
    );
  }),
);

/** The smallest atlas scale that does not need enlarging, or the largest one if all do. */
function sharpestScaleFor(pixelZoom: number): number {
  return atlas.scales.find((scale) => scale >= pixelZoom - 0.01) ?? atlas.scales[atlas.scales.length - 1];
}

function framesOf(sprite: string, view: string, animation: string): string[] {
  const views = required(atlas.sprites[sprite], `sprite ${sprite}`).views;
  return required(required(views[view] ?? views.any, `view ${view} of ${sprite}`)[animation], `animation ${animation} of ${sprite}`);
}
function firstAnimation(sprite: string, view: string): string {
  const views = atlas.sprites[sprite].views;
  return Object.keys(views[view] ?? views.any)[0];
}

// ---------- The park layout, in world tiles ----------

interface Placed { sprite: string; x: number; y: number; orientation: number; animated?: boolean }

const isOnIsland = (x: number, y: number): boolean => {
  const fromEdgeX = Math.min(x, ISLAND_WIDTH - 1 - x);
  const fromEdgeY = Math.min(y, ISLAND_DEPTH - 1 - y);
  if (fromEdgeX < 0 || fromEdgeY < 0) return false;
  return fromEdgeX + fromEdgeY >= 3;
};

const pathKeys = new Set<string>();
const tileKey = (x: number, y: number): string => `${x},${y}`;
const addPathRectangle = (x0: number, y0: number, x1: number, y1: number): void => {
  for (let x = x0; x <= x1; x++) for (let y = y0; y <= y1; y++) pathKeys.add(tileKey(x, y));
};
addPathRectangle(0, 10, 23, 10);
addPathRectangle(8, 5, 8, 16);
addPathRectangle(17, 5, 17, 16);
addPathRectangle(8, 5, 17, 5);
addPathRectangle(8, 16, 17, 16);
addPathRectangle(11, 8, 14, 12);

const blockedTiles = new Set([tileKey(12, 10), tileKey(13, 10), tileKey(12, 9), tileKey(13, 9)]);

const placedObjects: Placed[] = [
  { sprite: "buildings/park_gate", x: 2.5, y: 10.5, orientation: 0 },
  { sprite: "buildings/ticket_booth", x: 6, y: 12, orientation: 3 },
  { sprite: "buildings/souvenir_shop", x: 6.5, y: 6.5, orientation: 0 },
  { sprite: "buildings/burger_stall", x: 11, y: 4, orientation: 1 },
  { sprite: "buildings/restroom", x: 14.5, y: 4, orientation: 1 },
  { sprite: "buildings/burger_stall", x: 19, y: 13, orientation: 2 },
  { sprite: "buildings/souvenir_shop", x: 12.5, y: 18.5, orientation: 3 },
  { sprite: "scenery/fountain", x: 13, y: 10, orientation: 0, animated: true },
];
const scatter = (sprite: string, orientation: number, tiles: readonly (readonly [number, number])[]): void => {
  for (const [x, y] of tiles) placedObjects.push({ sprite, x: x + 0.5, y: y + 0.5, orientation });
};
scatter("scenery/lamp", 0, [[7, 9]]);
scatter("scenery/lamp", 2, [[18, 9], [18, 11]]);
scatter("scenery/lamp", 1, [[12, 7], [16, 11]]);
scatter("scenery/lamp", 3, [[11, 13], [15, 13]]);
scatter("scenery/bench", 1, [[10, 6], [13, 6], [16, 6]]);
scatter("scenery/bench", 3, [[10, 15], [15, 15]]);
scatter("scenery/bench", 2, [[16, 13]]);
scatter("scenery/bin", 0, [[9, 6], [18, 12], [9, 15], [16, 15], [12, 13]]);
scatter("scenery/flower_bed", 0, [[10, 13], [15, 7], [9, 8], [16, 9], [20, 8], [4, 11]]);
scatter("scenery/signpost", 0, [[9, 11]]);
scatter("scenery/balloon_cart", 1, [[14, 13]]);
scatter(theme.props.mushrooms ? "scenery/mushroom_stool" : "scenery/planter", 0, [[20, 16], [21, 15], [6, 17], [2, 14]]);
// Every theme has seven trees; each takes the same spots in the park.
const TREE_SPOTS: readonly (readonly [number, number])[][] = [
  [[3, 7], [20, 4], [10, 9], [7, 14], [18, 17], [22, 12]],
  [[1, 8], [20, 6], [15, 9], [3, 15], [16, 18], [22, 7]],
  [[19, 3], [9, 13], [15, 14], [4, 13], [20, 15], [7, 3]],
  [[18, 3], [21, 9], [5, 15], [9, 17], [3, 4]],
  [[6, 2], [22, 9], [20, 11], [7, 17]],
  [[10, 1], [19, 18], [1, 15], [10, 12]],
  [[17, 2], [15, 1], [21, 5], [4, 3], [1, 13], [8, 18], [20, 17]],
];
if (theme.trees.length !== TREE_SPOTS.length) throw new Error(`Theme ${theme.name} needs ${TREE_SPOTS.length} trees`);
theme.trees.forEach((tree, index) => scatter(`scenery/${tree.id}`, 0, TREE_SPOTS[index]));
scatter("scenery/bush_round", 0, [[5, 9], [19, 8], [11, 15], [14, 15], [10, 7], [5, 4], [18, 14]]);
scatter("scenery/bush_flowering", 0, [[7, 11], [19, 11], [13, 15], [13, 7], [18, 7], [3, 9]]);
for (let x = 18; x <= 22; x++) placedObjects.push({ sprite: "scenery/fence", x: x + 0.5, y: 8.5, orientation: 1 });
for (let y = 4; y <= 8; y++) placedObjects.push({ sprite: "scenery/fence", x: 21.5, y: y + 0.5, orientation: 0 });

for (const { sprite, x, y } of placedObjects) {
  if (sprite.startsWith("scenery/") && sprite !== "scenery/fountain" && pathKeys.has(tileKey(Math.floor(x), Math.floor(y)))) {
    throw new Error(`${sprite} at ${x},${y} is standing on a path`);
  }
}

// ---------- People walking along paths ----------

const personSprites = Object.keys(atlas.sprites).filter((sprite) => sprite.startsWith("people/"));

interface Walker {
  sprite: string;
  cycleLength: number;
  x: number;
  y: number;
  targetX: number;
  targetY: number;
  heading: number;
  lane: number;
  distance: number;
  pauseSeconds: number;
  speed: number;
}

let walkerRandomState = 31337;
const walkerRandom = (): number => {
  walkerRandomState = (walkerRandomState * 16807) % 2147483647;
  return walkerRandomState / 2147483647;
};
const isWalkable = (x: number, y: number): boolean => pathKeys.has(tileKey(x, y)) && !blockedTiles.has(tileKey(x, y));
const walkableTiles = [...pathKeys].map((key) => key.split(",").map(Number) as [number, number]).filter(([x, y]) => isWalkable(x, y));

const chooseHeading = (x: number, y: number, currentHeading: number): number => {
  const options = [0, 1, 2, 3].filter((heading) => isWalkable(x + HEADING_STEPS[heading][0], y + HEADING_STEPS[heading][1]));
  const forward = options.filter((heading) => heading !== (currentHeading + 2) % 4);
  const pool = forward.length ? forward : options;
  if (pool.includes(currentHeading) && walkerRandom() < 0.72) return currentHeading;
  return pool[Math.floor(walkerRandom() * pool.length)];
};

const walkers: Walker[] = Array.from({ length: 48 }, (_, index) => {
  const sprite = personSprites[index % personSprites.length];
  const [tileX, tileY] = walkableTiles[Math.floor(walkerRandom() * walkableTiles.length)];
  const heading = chooseHeading(tileX, tileY, Math.floor(walkerRandom() * 4));
  const cycleLength = Number(atlas.frames[framesOf(sprite, "se", "walk")[0]].properties["walk-cycle-length"]);
  return {
    sprite,
    cycleLength,
    x: tileX,
    y: tileY,
    targetX: tileX + HEADING_STEPS[heading][0],
    targetY: tileY + HEADING_STEPS[heading][1],
    heading,
    lane: (walkerRandom() - 0.5) * 0.5,
    distance: walkerRandom() * 100,
    pauseSeconds: 0,
    speed: 0.48 + walkerRandom() * 0.14,
  };
});

function stepWalkers(deltaSeconds: number): void {
  for (const walker of walkers) {
    if (walker.pauseSeconds > 0) {
      walker.pauseSeconds -= deltaSeconds;
      continue;
    }
    const step = walker.speed * deltaSeconds;
    walker.distance += step * TILE_EDGE_UNITS;
    const remaining = Math.hypot(walker.targetX - walker.x, walker.targetY - walker.y);
    if (remaining > step) {
      walker.x += ((walker.targetX - walker.x) / remaining) * step;
      walker.y += ((walker.targetY - walker.y) / remaining) * step;
      continue;
    }
    walker.x = walker.targetX;
    walker.y = walker.targetY;
    walker.heading = chooseHeading(walker.x, walker.y, walker.heading);
    walker.targetX = walker.x + HEADING_STEPS[walker.heading][0];
    walker.targetY = walker.y + HEADING_STEPS[walker.heading][1];
    if (walkerRandom() < 0.1) walker.pauseSeconds = 1 + walkerRandom() * 3;
  }
}

// ---------- WebGL sprite batcher ----------

const canvas = element<HTMLCanvasElement>("scene");
const gl = required(canvas.getContext("webgl2", { alpha: false, antialias: false, preserveDrawingBuffer: true }), "WebGL 2");

function compileProgram(vertexSource: string, fragmentSource: string): WebGLProgram {
  const program = required(gl.createProgram(), "program");
  for (const [type, source] of [[gl.VERTEX_SHADER, vertexSource], [gl.FRAGMENT_SHADER, fragmentSource]] as const) {
    const shader = required(gl.createShader(type), "shader");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(required(gl.getShaderInfoLog(shader), "shader log"));
    gl.attachShader(program, shader);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(required(gl.getProgramInfoLog(program), "program log"));
  return program;
}

const grainGlsl = `
  float paperGrain(vec2 pixel) {
    vec2 cell = floor(pixel / uDensity);
    float noise = fract(sin(dot(cell, vec2(12.9898, 78.233))) * 43758.5453);
    return 1.0 - uGrain * (noise - 0.5) * 0.05;
  }`;

const spriteProgram = compileProgram(
  `#version 300 es
  in vec2 aPosition; in vec2 aTexture;
  uniform vec2 uResolution;
  out vec2 vTexture;
  void main() {
    vTexture = aTexture;
    gl_Position = vec4(aPosition.x / uResolution.x * 2.0 - 1.0, 1.0 - aPosition.y / uResolution.y * 2.0, 0.0, 1.0);
  }`,
  `#version 300 es
  precision highp float;
  in vec2 vTexture;
  uniform sampler2D uAtlas;
  uniform float uShimmerTime; uniform float uShimmer; uniform float uGrain; uniform float uDensity; uniform vec2 uTexel;
  out vec4 color;
  ${grainGlsl}
  void main() {
    // Low-frequency sideways drift that changes a few times a second, as line boil does in hand-drawn animation.
    vec2 drift = vec2(sin(gl_FragCoord.y * 0.09 / uDensity + uShimmerTime * 2.4), cos(gl_FragCoord.x * 0.11 / uDensity + uShimmerTime * 1.7));
    color = texture(uAtlas, vTexture + drift * uShimmer * uTexel);
    color.rgb *= paperGrain(gl_FragCoord.xy);
  }`,
);
const spriteLocations = {
  position: gl.getAttribLocation(spriteProgram, "aPosition"),
  texture: gl.getAttribLocation(spriteProgram, "aTexture"),
  resolution: gl.getUniformLocation(spriteProgram, "uResolution"),
  shimmerTime: gl.getUniformLocation(spriteProgram, "uShimmerTime"),
  shimmer: gl.getUniformLocation(spriteProgram, "uShimmer"),
  grain: gl.getUniformLocation(spriteProgram, "uGrain"),
  density: gl.getUniformLocation(spriteProgram, "uDensity"),
  texel: gl.getUniformLocation(spriteProgram, "uTexel"),
};

const skyProgram = compileProgram(
  `#version 300 es
  in vec2 aPosition; out vec2 vPosition;
  void main() { vPosition = aPosition; gl_Position = vec4(aPosition, 0.0, 1.0); }`,
  `#version 300 es
  precision highp float;
  in vec2 vPosition; uniform float uGrain; uniform float uDensity; uniform vec3 uSkyTop; uniform vec3 uSkyMiddle; uniform vec3 uSkyBottom; out vec4 color;
  ${grainGlsl}
  void main() {
    float height = vPosition.y * 0.5 + 0.5;
    vec3 top = uSkyTop, middle = uSkyMiddle, bottom = uSkyBottom;
    vec3 sky = height > 0.45 ? mix(middle, top, (height - 0.45) / 0.55) : mix(bottom, middle, height / 0.45);
    color = vec4(sky * paperGrain(gl_FragCoord.xy), 1.0);
  }`,
);
const skyGrain = gl.getUniformLocation(skyProgram, "uGrain");
const skyColors = (["uSkyTop", "uSkyMiddle", "uSkyBottom"] as const).map((name) => gl.getUniformLocation(skyProgram, name));
const rgbOf = (hexColor: string): [number, number, number] => [1, 3, 5].map((start) => parseInt(hexColor.slice(start, start + 2), 16) / 255) as [number, number, number];
const skyDensity = gl.getUniformLocation(skyProgram, "uDensity");
const fullScreenTriangle = required(gl.createBuffer(), "buffer");
gl.bindBuffer(gl.ARRAY_BUFFER, fullScreenTriangle);
gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);

const textures: Record<string, WebGLTexture[]> = {};
gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
for (const scale of atlas.scales) {
  textures[scale] = pageImages[scale].map((image) => {
    const texture = required(gl.createTexture(), "texture");
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return texture;
  });
}

const FLOATS_PER_VERTEX = 4;
const MAX_SPRITES = 8000;
const vertexData = new Float32Array(MAX_SPRITES * 6 * FLOATS_PER_VERTEX);
const vertexBuffer = required(gl.createBuffer(), "buffer");
gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
gl.bufferData(gl.ARRAY_BUFFER, vertexData.byteLength, gl.DYNAMIC_DRAW);

interface Command { depth: number; key: string; screenX: number; screenY: number }

const camera = { rotation: 0, zoom: 1, panX: 0, panY: 0, shimmer: true, grain: true };
let drawnSprites = 0;
let drawCalls = 0;

function rotateAroundCenter(x: number, y: number, rotation: number): [number, number] {
  const [relativeX, relativeY] = [x - ISLAND_WIDTH / 2, y - ISLAND_DEPTH / 2];
  switch (rotation) {
    case 1: return [-relativeY, relativeX];
    case 2: return [-relativeX, -relativeY];
    case 3: return [relativeY, -relativeX];
    default: return [relativeX, relativeY];
  }
}

function screenOf(viewX: number, viewY: number): [number, number] {
  return [(viewX - viewY) * TILE_HALF_WIDTH, (viewX + viewY) * TILE_HALF_HEIGHT];
}

function groundCommands(rotation: number): Command[] {
  const commands: Command[] = [];
  for (let x = 0; x < ISLAND_WIDTH; x++) {
    for (let y = 0; y < ISLAND_DEPTH; y++) {
      if (!isOnIsland(x, y)) continue;
      const [viewX, viewY] = rotateAroundCenter(x + 0.5, y + 0.5, rotation);
      const [screenX, screenY] = screenOf(viewX, viewY);
      const depth = viewX + viewY;
      const isPath = pathKeys.has(tileKey(x, y));
      let sprite: string;
      if (isPath) {
        const connected = [0, 1, 2, 3].map((heading) => pathKeys.has(tileKey(x + HEADING_STEPS[heading][0], y + HEADING_STEPS[heading][1])));
        const connectedInView = [0, 1, 2, 3].map((viewHeading) => connected[(viewHeading - rotation + 4) % 4]);
        sprite = `ground/path_${connectedInView.map(Number).join("")}`;
      } else {
        const shade = (x + y) % 2 ? "light" : "dark";
        sprite = `ground/grass_${shade}_${(x * 5 + y * 3) % 3}`;
      }
      commands.push({ depth, key: framesOf(sprite, "any", "static")[0], screenX, screenY });
      // Cliff faces below edges that border the void, as seen by the current camera.
      const hasGroundToward = (viewHeading: number): boolean => {
        const worldHeading = (viewHeading - rotation + 4) % 4;
        return isOnIsland(x + HEADING_STEPS[worldHeading][0], y + HEADING_STEPS[worldHeading][1]);
      };
      const variant = (x * 3 + y * 5) % 3;
      if (!hasGroundToward(0)) {
        // Below the +x edge, hanging from the tile's right corner.
        commands.push({ depth: depth - 0.2, key: framesOf(`ground/cliff_right_${variant}`, "any", "static")[0], screenX: screenX + TILE_HALF_WIDTH, screenY });
      }
      if (!hasGroundToward(1)) {
        commands.push({ depth: depth - 0.2, key: framesOf(`ground/cliff_left_${variant}`, "any", "static")[0], screenX: screenX - TILE_HALF_WIDTH, screenY });
      }
    }
  }
  return commands.sort((first, second) => first.depth - second.depth);
}

function objectCommands(rotation: number, seconds: number): Command[] {
  const commands: Command[] = placedObjects.map((object) => {
    const [viewX, viewY] = rotateAroundCenter(object.x, object.y, rotation);
    const [screenX, screenY] = screenOf(viewX, viewY);
    const view = ROTATION_NAMES[(object.orientation + rotation) % 4];
    const frames = framesOf(object.sprite, view, firstAnimation(object.sprite, view));
    return { depth: viewX + viewY, key: frames[object.animated ? Math.floor(seconds * 7) % frames.length : 0], screenX, screenY };
  });
  for (const walker of walkers) {
    const [sideX, sideY] = HEADING_STEPS[(walker.heading + 1) % 4];
    const [worldX, worldY] = [walker.x + 0.5 + sideX * walker.lane, walker.y + 0.5 + sideY * walker.lane];
    const [viewX, viewY] = rotateAroundCenter(worldX, worldY, rotation);
    const [screenX, screenY] = screenOf(viewX, viewY);
    const facing = FACING_NAMES[(walker.heading + rotation) % 4];
    const isPaused = walker.pauseSeconds > 0;
    const frames = framesOf(walker.sprite, facing, isPaused ? "idle" : "walk");
    const frame = isPaused
      ? Math.floor(seconds * 1.3 + walker.lane * 10) % frames.length
      : Math.floor((walker.distance / walker.cycleLength) * frames.length) % frames.length;
    commands.push({ depth: viewX + viewY, key: frames[Math.abs(frame)], screenX, screenY });
  }
  return commands.sort((first, second) => first.depth - second.depth);
}

function fixedSkyCommands(width: number, height: number, seconds: number, density: number): Command[] {
  const layout = (factorX: number, factorY: number): [number, number] => [width * factorX / density, height * factorY / density];
  const drift = (speed: number, span: number, offset: number): number => ((seconds * speed + offset) % span) - span * 0.1;
  const place = (sprite: string, [x, y]: [number, number]): Command => ({ depth: 0, key: framesOf(sprite, "any", "static")[0], screenX: x, screenY: y });
  return [
    place("sky/sun", layout(0.1, 0.14)),
    place("sky/cloud_big", [drift(4, width / density * 1.2, 180), height * 0.1 / density]),
    place("sky/cloud_small", [drift(6, width / density * 1.2, 700), height * 0.2 / density]),
    place("sky/cloud_small", [drift(5, width / density * 1.2, 1100), height * 0.07 / density]),
  ];
}

function drawBatches(commands: readonly Command[], zoom: number, density: number, originX: number, originY: number, atlasScale: number): void {
  const pixelZoom = zoom * density;
  let usedFloats = 0;
  let currentPage = -1;
  const flush = (): void => {
    if (usedFloats === 0) return;
    gl.bindTexture(gl.TEXTURE_2D, textures[atlasScale][currentPage]);
    const page = atlas.pages[atlasScale][currentPage];
    gl.uniform2f(spriteLocations.texel, 1 / page.width, 1 / page.height);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, vertexData, 0, usedFloats);
    gl.drawArrays(gl.TRIANGLES, 0, usedFloats / FLOATS_PER_VERTEX);
    drawCalls++;
    usedFloats = 0;
  };
  for (const { key, screenX, screenY } of commands) {
    const frame = atlas.frames[key];
    const placement = frame.placements[atlasScale];
    if (placement.page !== currentPage) {
      flush();
      currentPage = placement.page;
    }
    const page = atlas.pages[atlasScale][placement.page];
    // Snap sprite origins to whole device pixels, so that sprites stay sharp at integer zoom.
    const left = Math.round(originX + (screenX - frame.anchor.x) * pixelZoom);
    const top = Math.round(originY + (screenY - frame.anchor.y) * pixelZoom);
    const right = left + frame.width * pixelZoom;
    const bottom = top + frame.height * pixelZoom;
    const [u0, v0, u1, v1] = [placement.x / page.width, placement.y / page.height, (placement.x + placement.width) / page.width, (placement.y + placement.height) / page.height];
    vertexData.set([left, top, u0, v0, right, top, u1, v0, left, bottom, u0, v1, left, bottom, u0, v1, right, top, u1, v0, right, bottom, u1, v1], usedFloats);
    usedFloats += 6 * FLOATS_PER_VERTEX;
    drawnSprites++;
  }
  flush();
}

function drawScene(seconds: number): void {
  const density = window.devicePixelRatio || 1;
  const [width, height] = [Math.round(canvas.clientWidth * density), Math.round(canvas.clientHeight * density)];
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  gl.viewport(0, 0, width, height);
  drawnSprites = 0;
  drawCalls = 0;

  gl.disable(gl.BLEND);
  gl.useProgram(skyProgram);
  gl.uniform1f(skyGrain, camera.grain ? theme.scene.grain : 0);
  theme.scene.sky.forEach((skyColor, index) => gl.uniform3f(skyColors[index], ...rgbOf(skyColor)));
  gl.uniform1f(skyDensity, density);
  gl.bindBuffer(gl.ARRAY_BUFFER, fullScreenTriangle);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(gl.getAttribLocation(skyProgram, "aPosition"), 2, gl.FLOAT, false, 0, 0);
  gl.drawArrays(gl.TRIANGLES, 0, 3);

  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  gl.useProgram(spriteProgram);
  gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
  gl.enableVertexAttribArray(spriteLocations.position);
  gl.enableVertexAttribArray(spriteLocations.texture);
  gl.vertexAttribPointer(spriteLocations.position, 2, gl.FLOAT, false, FLOATS_PER_VERTEX * 4, 0);
  gl.vertexAttribPointer(spriteLocations.texture, 2, gl.FLOAT, false, FLOATS_PER_VERTEX * 4, 8);
  gl.uniform2f(spriteLocations.resolution, width, height);
  gl.uniform1f(spriteLocations.grain, camera.grain ? theme.scene.grain : 0);
  gl.uniform1f(spriteLocations.density, density);

  const pixelZoom = camera.zoom * density;
  const atlasScale = sharpestScaleFor(pixelZoom);
  // The shimmer steps a few times a second, and stays under about a third of a screen pixel.
  gl.uniform1f(spriteLocations.shimmerTime, Math.floor(seconds * 6));
  gl.uniform1f(spriteLocations.shimmer, 0);
  drawBatches(fixedSkyCommands(width, height, seconds, density), 1, density, 0, 0, sharpestScaleFor(density));
  const originX = width / 2 + camera.panX * density;
  const originY = height / 2 - 30 * density + camera.panY * density;
  drawBatches(groundCommands(camera.rotation), camera.zoom, density, originX, originY, atlasScale);
  gl.uniform1f(spriteLocations.shimmer, camera.shimmer ? (0.32 * atlasScale) / pixelZoom : 0);
  drawBatches(objectCommands(camera.rotation, seconds), camera.zoom, density, originX, originY, atlasScale);
}

const statsElement = element("stats");
let previousMilliseconds = performance.now();
let frameCounter = 0;
let statsTimer = previousMilliseconds;
function animate(nowMilliseconds: number): void {
  stepWalkers(Math.min(0.1, (nowMilliseconds - previousMilliseconds) / 1000));
  previousMilliseconds = nowMilliseconds;
  drawScene(nowMilliseconds / 1000);
  frameCounter++;
  if (nowMilliseconds - statsTimer > 1000) {
    statsElement.textContent = `${frameCounter} fps, ${drawnSprites} sprites, ${drawCalls} draw calls`;
    frameCounter = 0;
    statsTimer = nowMilliseconds;
  }
  requestAnimationFrame(animate);
}
requestAnimationFrame(animate);

// ---------- Scene controls ----------

element("rotate-left").addEventListener("click", () => { camera.rotation = (camera.rotation + 3) % 4; });
element("rotate-right").addEventListener("click", () => { camera.rotation = (camera.rotation + 1) % 4; });
const zoomButtons = element("zoom-buttons");
for (const zoom of [0.5, 0.75, 1, 1.5, 2, 3]) {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.zoom = String(zoom);
  button.textContent = `${zoom}x`;
  button.setAttribute("aria-pressed", String(zoom === camera.zoom));
  button.addEventListener("click", () => {
    camera.zoom = zoom;
    zoomButtons.querySelectorAll("button").forEach((other) => other.setAttribute("aria-pressed", String(other === button)));
  });
  zoomButtons.append(button);
}
for (const effect of ["shimmer", "grain"] as const) {
  const button = element(effect);
  button.addEventListener("click", () => {
    camera[effect] = !camera[effect];
    button.setAttribute("aria-pressed", String(camera[effect]));
  });
}
let dragStart: { x: number; y: number; panX: number; panY: number } | undefined;
canvas.addEventListener("pointerdown", (event) => {
  dragStart = { x: event.clientX, y: event.clientY, panX: camera.panX, panY: camera.panY };
  canvas.setPointerCapture(event.pointerId);
});
canvas.addEventListener("pointermove", (event) => {
  if (!dragStart) return;
  camera.panX = dragStart.panX + event.clientX - dragStart.x;
  camera.panY = dragStart.panY + event.clientY - dragStart.y;
});
canvas.addEventListener("pointerup", () => { dragStart = undefined; });

// ---------- Gallery ----------

const GALLERY_CATEGORIES = ["people", "buildings", "scenery", "ground", "sky"];
const FACING_ORDER = ["se", "sw", "nw", "ne"];
const gallery = element("gallery");
let galleryRotation = 0;
let galleryCategory = "people";

interface GalleryItem { canvas: HTMLCanvasElement; sprite: string; zoom: number; fixedView?: string }
const galleryItems: GalleryItem[] = [];

function makeSpriteCanvas(sprite: string, zoom: number, fixedView?: string): HTMLCanvasElement {
  const views = atlas.sprites[sprite].views;
  const view = fixedView ?? Object.keys(views)[0];
  const frame = atlas.frames[framesOf(sprite, view, firstAnimation(sprite, view))[0]];
  const spriteCanvas = document.createElement("canvas");
  spriteCanvas.className = "sprite";
  spriteCanvas.style.width = `${frame.width * zoom}px`;
  spriteCanvas.style.height = `${frame.height * zoom}px`;
  const density = window.devicePixelRatio || 1;
  spriteCanvas.width = Math.round(frame.width * zoom * density);
  spriteCanvas.height = Math.round(frame.height * zoom * density);
  galleryItems.push({ canvas: spriteCanvas, sprite, zoom, fixedView });
  return spriteCanvas;
}

function currentGalleryView(sprite: string): string {
  const views = atlas.sprites[sprite].views;
  if (views.any) return "any";
  if (views.se) return FACING_ORDER[galleryRotation];
  return ROTATION_NAMES[galleryRotation];
}

function buildGallery(): void {
  gallery.replaceChildren();
  galleryItems.length = 0;
  for (const sprite of Object.keys(atlas.sprites).filter((name) => name.startsWith(`${galleryCategory}/`))) {
    const views = Object.keys(atlas.sprites[sprite].views);
    const card = document.createElement("article");
    card.className = "card";
    card.dataset.sprite = sprite;
    const title = document.createElement("h3");
    title.textContent = sprite.split("/")[1].replaceAll("_", " ");
    const note = document.createElement("small");
    const frameKey = framesOf(sprite, views[0], firstAnimation(sprite, views[0]))[0];
    note.textContent = atlas.frames[frameKey].properties.description ?? "";
    const stage = document.createElement("div");
    stage.className = "stage";
    for (const [zoom, label] of [[1, "game size"], [3, "3x zoom"]] as const) {
      const figure = document.createElement("figure");
      figure.append(makeSpriteCanvas(sprite, zoom), label);
      stage.append(figure);
    }
    // Large sprites get a full-width card so that the 3x copy fits.
    if (atlas.frames[frameKey].width * 3 > 300) card.style.gridColumn = "1 / -1";
    card.append(title, note, stage);
    if (views.length > 1) {
      const sides = document.createElement("div");
      sides.className = "sides";
      const order = atlas.sprites[sprite].views.se ? FACING_ORDER : ROTATION_NAMES;
      for (const view of order) sides.append(makeSpriteCanvas(sprite, 1, view));
      card.append(sides);
    }
    gallery.append(card);
  }
}

function drawGallery(seconds: number): void {
  const density = window.devicePixelRatio || 1;
  for (const { canvas: spriteCanvas, sprite, zoom, fixedView } of galleryItems) {
    const view = fixedView ?? currentGalleryView(sprite);
    const animation = atlas.sprites[sprite].views[view]?.walk ? "walk" : firstAnimation(sprite, view);
    const frames = framesOf(sprite, view, animation);
    const frame = atlas.frames[frames[Math.floor(seconds * 10) % frames.length]];
    const sourceScale = sharpestScaleFor(zoom * density);
    const placement = frame.placements[sourceScale];
    const context = required(spriteCanvas.getContext("2d"), "2D context");
    context.imageSmoothingQuality = "high";
    context.clearRect(0, 0, spriteCanvas.width, spriteCanvas.height);
    context.drawImage(pageImages[sourceScale][placement.page], placement.x, placement.y, placement.width, placement.height, 0, 0, spriteCanvas.width, spriteCanvas.height);
  }
}

const categoryButtons = element("category-buttons");
for (const category of GALLERY_CATEGORIES) {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.category = category;
  button.textContent = category;
  button.setAttribute("aria-pressed", String(category === galleryCategory));
  button.addEventListener("click", () => {
    galleryCategory = category;
    categoryButtons.querySelectorAll("button").forEach((other) => other.setAttribute("aria-pressed", String(other === button)));
    buildGallery();
  });
  categoryButtons.append(button);
}
element("gallery-left").addEventListener("click", () => { galleryRotation = (galleryRotation + 3) % 4; });
element("gallery-right").addEventListener("click", () => { galleryRotation = (galleryRotation + 1) % 4; });
buildGallery();
window.setInterval(() => drawGallery(performance.now() / 1000), 66);
document.documentElement.dataset.ready = "true";
