import type { Camera, Viewport } from "../camera";
import { gradeColor, lightingAt, NOON, type Lighting, type Rgb } from "./daylight";
import type { Glow } from "./effects";
import { clipNightLightsAboveLevel } from "./effects";
import { createRenderResources, FLOATS_PER_VERTEX, INDICES_PER_QUAD, VERTICES_PER_QUAD } from "./resources";
import { cutawayBounds, CutSide, cutSideVisibleAt, polygonTouchesCutaway, type Cutaway } from "./cutaway";
import { buildGridQuads } from "./grid";
import { appendGeometry, buildScene, clipGeometryAboveLevel, pickScene, SHAPE_PAGES, UNDERGROUND_CONTENT_DEPTH_BIAS, UNPICKABLE_KINDS, type Atlas, type GeometryQuad, type Pick, type Scene, type Triangle, type Vertex, type WorldView } from "./scene";
export type { Atlas, GeometryQuad, Pick, WorldSprite, WorldView } from "./scene";
export type { Cutaway } from "./cutaway";
export { terrainHeightAt } from "./scene";
export { clipGeometryAboveLevel } from "./scene";
const NOON_LIGHTING = lightingAt(NOON);

export async function loadAtlas(url: string): Promise<{ atlas: Atlas; images: Record<string, HTMLImageElement[]> }> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Could not load atlas: ${response.status}`);
  const atlas = await response.json() as Atlas;
  if (!atlas.pages["1"]?.length) throw new Error("Atlas has no 1x pages");
  const images = Object.fromEntries(await Promise.all(Object.entries(atlas.pages).map(async ([scale, pages]) => [scale, await Promise.all(pages.map(async (page) => {
    const image = new Image();
    image.src = new URL(page.file, new URL(url, document.baseURI)).href;
    await image.decode();
    return image;
  }))])));
  return { atlas, images };
}

export interface WorldRenderer {
  /** Without an atmosphere, as in the isolated rendering fixtures, the world is drawn at noon over a plain background with no effects. */
  render(input: RenderInput): void;
  isContextAvailable(): boolean;
  pick(x: number, y: number): Pick | null;
  /** Picks a row-major rectangle in backing-store pixels. Pixels outside the canvas are null. */
  pickRegion(x: number, y: number, width: number, height: number): (Pick | null)[];
  pickSurface(x: number, y: number): Pick | null;
  /** Picks the original surface beneath the pointer without applying the cutaway window. */
  pickSurfaceUncut(x: number, y: number): Pick | null;
  pickFacilityPlacement(x: number, y: number): Pick | null;
  waitForGpu(): void;
}

export interface RenderInput {
  world: WorldView;
  camera: Camera;
  waterFrame?: number;
  /** Geometry is clipped to the underground level and participates in depth testing and picking. */
  geometryPasses?: readonly GeometryPass[];
  atmospherePass?: AtmospherePass;
  /** Show clearer tile borders while a construction tool is active. */
  showBuildGrid?: boolean;
  /** Draw the world cut at a level inside a round window and trace buried coaster track outside it. Not combined with `world.undergroundLevel`. */
  cutaway?: Cutaway;
}

function validatePickRegion(x: number, y: number, width: number, height: number): void {
  if (![x, y, width, height].every(Number.isSafeInteger) || width < 1 || height < 1 || width * height > 1_000_000) {
    throw new RangeError("Invalid picking region");
  }
}

export type GeometryPass = (camera: Camera, viewport: Viewport, atlas: Atlas, scale: string) => readonly GeometryQuad[];

/** Everything the time of day adds around the world. Quads in `sky`, `overlay` and `sparks` are in canvas pixels and are not depth tested. */
export interface Atmosphere {
  lighting: Lighting;
  /** Where the sky gives way to paper below the park, in canvas pixels from the top. */
  horizonY: number;
  /** The paper below the horizon, already graded for the time of day. */
  paper: Rgb;
  /** Sun, moon and clouds, behind the world. */
  sky: readonly GeometryQuad[];
  /** Ground shadows, over terrain and paths and under everything standing on them. */
  decals: readonly GeometryQuad[];
  /** Depth-tested world geometry that is never picked, such as bulbs. */
  lights: readonly GeometryQuad[];
  /** Lamp light, in canvas pixels, that relights whatever the world draws beneath it; see `lightInPool`. */
  pools: readonly GeometryQuad[];
  /** Confetti, blended over the world. */
  overlay: readonly GeometryQuad[];
  /** Firework sparks, added to the world. */
  sparks: readonly GeometryQuad[];
  glows: readonly Glow[];
}
export type AtmospherePass = (camera: Camera, viewport: Viewport, atlas: Atlas, scale: string) => Atmosphere;

/** Surfaces that stay partly lit at night, as if by path lamps, so a busy park stays readable. */
const PATH_NIGHT_LIGHT = 0.22;
const GUEST_NIGHT_LIGHT = 0.45;
/** Coasters keep part of their paint at night, as if floodlit, so each coaster's colours still read after dark. */
const COASTER_NIGHT_LIGHT: Readonly<Record<string, number>> = { track: 0.4, train: 0.55, support: 0.2 };
const GROUND_KINDS: ReadonlySet<string> = new Set(["terrain", "cliff", "water", "path", "carved-floor", "cavern-wall", "tunnel-portal"]);
const BURIED_TRACE_DAY = [0.16, 0.22, 0.42, 0.5] as const;
const BURIED_TRACE_NIGHT = [0.95, 0.85, 0.55, 0.5] as const;
/** The cutaway window's edge is inked dark by day and light at night so it reads against the ground either way. */
const CUTAWAY_RIM_DAY = [0.2, 0.15, 0.12] as const;
const CUTAWAY_RIM_NIGHT = [0.92, 0.83, 0.63] as const;
const GLOW_DOWNSCALE = 4;
const POOL_DOWNSCALE = 2;
/** The glow layer is printed slightly out of register with the world, in CSS pixels. */
const GLOW_MISREGISTRATION = { x: 2, y: -1.5 };
const shapeCodes: Record<number, number> = { [SHAPE_PAGES.flat]: 0, [SHAPE_PAGES.glow]: 1, [SHAPE_PAGES.disc]: 2, [SHAPE_PAGES.softDisc]: 3, [SHAPE_PAGES.lightPool]: 4 };

interface Batch { page: number; data: Float32Array; count: number }
interface QuadSource { vertices: readonly Vertex[]; page: number; color: Triangle["color"]; light: number; pickId: number; cut: CutSide }
/** Coaster geometry hidden by the ground is traced through it as a flat silhouette while the cutaway is open. */
const BURIED_TRACE_KINDS: ReadonlySet<string> = new Set(["track", "support", "train", "rider"]);

function nightLight(triangle: Triangle): number {
  if (triangle.light !== undefined) return triangle.light;
  if (triangle.kind === "path") return PATH_NIGHT_LIGHT;
  if (triangle.kind === "rider" || triangle.id?.startsWith("guest-")) return GUEST_NIGHT_LIGHT;
  return COASTER_NIGHT_LIGHT[triangle.kind] ?? 0;
}

/** Packs quads into vertex data, four vertices per quad, one batch per run of quads that share an atlas page or shape. */
function encode(quads: readonly QuadSource[]): Batch[] {
  const batches: Batch[] = [];
  let start = 0;
  while (start < quads.length) {
    const page = quads[start].page; let end = start + 1;
    while (end < quads.length && quads[end].page === page) end++;
    const data = new Float32Array((end - start) * VERTICES_PER_QUAD * FLOATS_PER_VERTEX);
    for (let i = start; i < end; i++) {
      const { vertices, color, light, pickId, cut } = quads[i];
      vertices.forEach((vertex, j) => {
        const offset = ((i - start) * VERTICES_PER_QUAD + j) * FLOATS_PER_VERTEX;
        data.set([vertex.x, vertex.y, vertex.depth, vertex.u, vertex.v, (pickId & 255) / 255, ((pickId >> 8) & 255) / 255, ((pickId >> 16) & 255) / 255], offset);
        data.set(color, offset + 8);
        data[offset + 12] = light;
        data[offset + 13] = cut;
      });
    }
    batches.push({ page, data, count: end - start }); start = end;
  }
  return batches;
}

/** Two triangles per quad over its four vertices, in the order 0-1-2 and 0-2-3. */
function quadIndices(quadCount: number): Uint32Array {
  const indices = new Uint32Array(quadCount * INDICES_PER_QUAD);
  for (let quad = 0; quad < quadCount; quad++) {
    const first = quad * VERTICES_PER_QUAD;
    indices.set([first, first + 1, first + 2, first, first + 2, first + 3], quad * INDICES_PER_QUAD);
  }
  return indices;
}

const fromGeometry = (quad: GeometryQuad, cut: CutSide = CutSide.everywhere): QuadSource => ({ vertices: quad.vertices, page: quad.page ?? SHAPE_PAGES.flat, color: quad.color, light: quad.light ?? 0, pickId: 0, cut });

function createWorldRendererInstance(canvas: HTMLCanvasElement, atlas: Atlas, images: Record<string, HTMLImageElement[]>, preserveDrawingBuffer: boolean): WorldRenderer {
  const gl = canvas.getContext("webgl2", { alpha: false, preserveDrawingBuffer, stencil: true });
  if (!gl) throw new Error("WebGL2 is unavailable");
  for (const [scale, pages] of Object.entries(atlas.pages)) if (images[scale]?.length !== pages.length) throw new Error("Atlas page count does not match images");
  const { program, skyProgram, glowProgram, vao, emptyVao, buffer, framebuffer, pickTexture, depthBuffer, glowFramebuffer, glowTexture, poolFramebuffer, poolTexture, textures } = createRenderResources(gl, images);
  let activeScale = "1";
  gl.useProgram(program); gl.uniform1i(gl.getUniformLocation(program, "atlas"), 0); gl.uniform1i(gl.getUniformLocation(program, "poolLight"), 1);
  gl.disable(gl.DITHER); gl.depthFunc(gl.LEQUAL);
  const uniform = (target: WebGLProgram, name: string): WebGLUniformLocation | null => gl!.getUniformLocation(target, name);
  let scene: Scene = { triangles: [] };
  let surfaceScene: Scene = { triangles: [] };
  let uncutSurfaceScene: Scene = { triangles: [] };
  let facilityScene: Scene = { triangles: [] };
  let groundBatches: Batch[] = []; let standingBatches: Batch[] = []; let buriedTraceBatches: Batch[] = [];
  let cutaway: Cutaway | undefined;
  let skyBatches: Batch[] = []; let decalBatches: Batch[] = []; let gridBatches: Batch[] = []; let overlayBatches: Batch[] = []; let sparkBatches: Batch[] = []; let glowBatches: Batch[] = []; let poolBatches: Batch[] = [];
  let atmosphere: Atmosphere | undefined;
  let depthMinimum = 0; let depthSpan = 2;
  let pickWidth = 0; let pickHeight = 0;
  let pickingDirty = true;
  let indexedQuads = 0;
  let glowWidth = 0; let glowHeight = 0;
  let poolWidth = 0; let poolHeight = 0;
  let pixelRatio = 1;
  function prepare(): void {
    let minimum = Infinity; let maximum = -Infinity;
    for (const triangle of scene.triangles) for (const vertex of triangle.vertices) { minimum = Math.min(minimum, vertex.depth); maximum = Math.max(maximum, vertex.depth); }
    depthMinimum = minimum - 1; depthSpan = Math.max(2, maximum - minimum + 2);
    const ground: QuadSource[] = []; const standing: QuadSource[] = []; const buriedTrace: QuadSource[] = [];
    for (let i = 0; i < scene.triangles.length / 2; i++) {
      const first = scene.triangles[i * 2]; const second = scene.triangles[i * 2 + 1];
      const quad = { vertices: [...first.vertices, second.vertices[2]], page: first.page, color: first.color, light: nightLight(first), pickId: UNPICKABLE_KINDS.has(first.kind) ? 0 : i + 1, cut: first.cut ?? CutSide.everywhere };
      (GROUND_KINDS.has(first.kind) ? ground : standing).push(quad);
      if (cutaway && quad.cut === CutSide.outside && BURIED_TRACE_KINDS.has(first.kind)) buriedTrace.push(quad);
    }
    groundBatches = encode(ground); standingBatches = encode(standing); buriedTraceBatches = encode(buriedTrace);
  }
  function setLighting(target: WebGLProgram, lighting: Lighting): void {
    gl!.uniform3fv(uniform(target, "gradeMultiply"), lighting.multiply); gl!.uniform3fv(uniform(target, "gradeShadowInk"), lighting.shadowInk);
    gl!.uniform3fv(uniform(target, "gradeHighlightInk"), lighting.highlightInk); gl!.uniform1f(uniform(target, "gradeInkAmount"), lighting.inkAmount);
    gl!.uniform1f(uniform(target, "gradeLightsOn"), lighting.lightsOn);
    gl!.uniform3fv(uniform(target, "cutawayRim"), lighting.lightsOn > 0.5 ? CUTAWAY_RIM_NIGHT : CUTAWAY_RIM_DAY);
  }
  function drawBatches(batches: readonly Batch[]): void {
    for (const { page, data, count } of batches) {
      if (count > indexedQuads) {
        indexedQuads = count;
        gl!.bufferData(gl!.ELEMENT_ARRAY_BUFFER, quadIndices(indexedQuads), gl!.STATIC_DRAW);
      }
      gl!.uniform1i(uniform(program, "textured"), page >= 0 ? 1 : 0);
      gl!.uniform1i(uniform(program, "shape"), page >= 0 ? 0 : shapeCodes[page]);
      gl!.bindTexture(gl!.TEXTURE_2D, page >= 0 ? textures[activeScale][page] : null); gl!.bufferData(gl!.ARRAY_BUFFER, data, gl!.DYNAMIC_DRAW);
      gl!.drawElements(gl!.TRIANGLES, count * INDICES_PER_QUAD, gl!.UNSIGNED_INT, 0);
    }
  }
  /** Overlays are drawn flat at `overlayDepth` in clip space: 0 to sit over everything, 1 to fill only what the world left empty. */
  function useWorldProgram(picking: boolean, overlay: boolean, graded: boolean, overlayDepth = 0, pooled = false): void {
    gl!.useProgram(program); gl!.uniform1i(uniform(program, "picking"), picking ? 1 : 0); gl!.uniform1i(uniform(program, "pooled"), pooled ? 1 : 0);
    gl!.uniform1i(uniform(program, "overlay"), overlay ? 1 : 0); gl!.uniform1f(uniform(program, "overlayDepth"), overlayDepth); gl!.uniform1i(uniform(program, "graded"), graded ? 1 : 0);
    gl!.uniform2f(uniform(program, "resolution"), canvas.width, canvas.height);
    gl!.uniform2f(uniform(program, "depthRange"), depthMinimum, depthSpan);
    // The shader compares against gl_FragCoord, which counts rows from the bottom.
    gl!.uniform3f(uniform(program, "cutaway"), cutaway?.x ?? 0, canvas.height - (cutaway?.y ?? 0), cutaway?.radius ?? 0);
    gl!.uniform4f(uniform(program, "silhouette"), 0, 0, 0, 0);
    gl!.bindVertexArray(vao); gl!.bindBuffer(gl!.ARRAY_BUFFER, buffer); gl!.activeTexture(gl!.TEXTURE0);
  }
  /** Entry: prepared batches and depth range. Exit: picking framebuffer bound, depth writes enabled and blending disabled. */
  function drawPicking(): void {
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer);
    gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.disable(gl!.SCISSOR_TEST); gl!.disable(gl!.CULL_FACE); gl!.disable(gl!.DITHER);
    gl!.colorMask(true, true, true, true); gl!.depthMask(true); gl!.depthFunc(gl!.LEQUAL);
    gl!.disable(gl!.BLEND); gl!.enable(gl!.DEPTH_TEST);
    gl!.clearColor(0, 0, 0, 0); gl!.clearDepth(1); gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT);
    useWorldProgram(true, false, false);
    drawBatches(groundBatches); drawBatches(standingBatches);
  }
  /** Entry: scissor disabled and color writes enabled. Exit: pool texture on unit 1 when present; otherwise unit 1 is unbound. */
  function drawPools(): boolean {
    // Unbound first, so the layer is never sampled while it is drawn into.
    gl!.activeTexture(gl!.TEXTURE1); gl!.bindTexture(gl!.TEXTURE_2D, null); gl!.activeTexture(gl!.TEXTURE0);
    if (!poolBatches.length) return false;
    const width = Math.ceil(canvas.width / POOL_DOWNSCALE); const height = Math.ceil(canvas.height / POOL_DOWNSCALE);
    if (width !== poolWidth || height !== poolHeight) {
      poolWidth = width; poolHeight = height;
      gl!.bindTexture(gl!.TEXTURE_2D, poolTexture); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.LINEAR); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.LINEAR);
      gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_S, gl!.CLAMP_TO_EDGE); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_T, gl!.CLAMP_TO_EDGE);
      gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA8, width, height, 0, gl!.RGBA, gl!.UNSIGNED_BYTE, null);
      gl!.bindFramebuffer(gl!.FRAMEBUFFER, poolFramebuffer); gl!.framebufferTexture2D(gl!.FRAMEBUFFER, gl!.COLOR_ATTACHMENT0, gl!.TEXTURE_2D, poolTexture, 0);
      if (gl!.checkFramebufferStatus(gl!.FRAMEBUFFER) !== gl!.FRAMEBUFFER_COMPLETE) throw new Error("Pool framebuffer is incomplete");
    }
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, poolFramebuffer); gl!.viewport(0, 0, width, height);
    gl!.clearColor(0, 0, 0, 0); gl!.clear(gl!.COLOR_BUFFER_BIT);
    gl!.disable(gl!.DEPTH_TEST); gl!.depthMask(false); gl!.enable(gl!.BLEND); gl!.blendEquation(gl!.FUNC_ADD); gl!.blendFunc(gl!.SRC_ALPHA, gl!.ONE);
    useWorldProgram(false, true, false); drawBatches(poolBatches);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null); gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.activeTexture(gl!.TEXTURE1); gl!.bindTexture(gl!.TEXTURE_2D, poolTexture); gl!.activeTexture(gl!.TEXTURE0);
    return true;
  }
  /** Picks on the CPU what the shader shows at the pixel, so the cutaway window picks the cut world. */
  function pickVisible(source: Scene, x: number, y: number): Pick | null {
    const pixelX = Math.floor(x) + 0.5; const pixelY = Math.floor(y) + 0.5;
    const visible = cutaway ? { triangles: source.triangles.filter((triangle) => cutSideVisibleAt(triangle.cut ?? CutSide.everywhere, cutaway, pixelX, pixelY)) } : source;
    return pickScene(visible, pixelX, pixelY, 0.125);
  }
  function resolvePick(pixel: Uint8Array, offset: number, x: number, y: number): Pick | null {
    const id = pixel[offset] + pixel[offset + 1] * 256 + pixel[offset + 2] * 65536;
    if (!id) return null;
    return pickScene({ triangles: scene.triangles.slice((id - 1) * 2, id * 2) }, x + 0.5, y + 0.5, 0.125);
  }
  function ensurePicking(): void {
    if (pickingDirty) {
      drawPicking();
      pickingDirty = false;
    }
  }
  /** Entry: scissor disabled and color writes enabled. Exit when drawn: default framebuffer, depth disabled and additive blending. */
  function drawGlows(): void {
    if (!glowBatches.length) return;
    const width = Math.ceil(canvas.width / GLOW_DOWNSCALE); const height = Math.ceil(canvas.height / GLOW_DOWNSCALE);
    if (width !== glowWidth || height !== glowHeight) {
      glowWidth = width; glowHeight = height;
      gl!.bindTexture(gl!.TEXTURE_2D, glowTexture); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.LINEAR); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.LINEAR);
      gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_S, gl!.CLAMP_TO_EDGE); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_T, gl!.CLAMP_TO_EDGE);
      gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA8, width, height, 0, gl!.RGBA, gl!.UNSIGNED_BYTE, null);
      gl!.bindFramebuffer(gl!.FRAMEBUFFER, glowFramebuffer); gl!.framebufferTexture2D(gl!.FRAMEBUFFER, gl!.COLOR_ATTACHMENT0, gl!.TEXTURE_2D, glowTexture, 0);
      if (gl!.checkFramebufferStatus(gl!.FRAMEBUFFER) !== gl!.FRAMEBUFFER_COMPLETE) throw new Error("Glow framebuffer is incomplete");
    }
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, glowFramebuffer); gl!.viewport(0, 0, width, height);
    gl!.clearColor(0, 0, 0, 0); gl!.clear(gl!.COLOR_BUFFER_BIT);
    gl!.disable(gl!.DEPTH_TEST); gl!.depthMask(false); gl!.enable(gl!.BLEND); gl!.blendEquation(gl!.FUNC_ADD); gl!.blendFunc(gl!.SRC_ALPHA, gl!.ONE);
    useWorldProgram(false, true, false); drawBatches(glowBatches);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null); gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.useProgram(glowProgram); gl!.bindVertexArray(emptyVao);
    gl!.activeTexture(gl!.TEXTURE0); gl!.bindTexture(gl!.TEXTURE_2D, glowTexture); gl!.uniform1i(uniform(glowProgram, "glow"), 0);
    gl!.uniform2f(uniform(glowProgram, "resolution"), canvas.width, canvas.height);
    gl!.uniform2f(uniform(glowProgram, "offset"), GLOW_MISREGISTRATION.x * pixelRatio, -GLOW_MISREGISTRATION.y * pixelRatio);
    gl!.blendFunc(gl!.ONE, gl!.ONE);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);
  }
  /** Entry: ground drawn with depth. Draws each pixel of coaster behind the ground once, as a flat silhouette. Exit: depth test LEQUAL with writes enabled. */
  function drawBuriedTrace(lighting: Lighting): void {
    gl!.enable(gl!.STENCIL_TEST); gl!.stencilFunc(gl!.EQUAL, 0, 0xff); gl!.stencilOp(gl!.KEEP, gl!.KEEP, gl!.INCR);
    gl!.depthFunc(gl!.GREATER); gl!.depthMask(false);
    const ink = lighting.lightsOn > 0.5 ? BURIED_TRACE_NIGHT : BURIED_TRACE_DAY;
    gl!.uniform4fv(uniform(program, "silhouette"), ink);
    drawBatches(buriedTraceBatches);
    gl!.uniform4f(uniform(program, "silhouette"), 0, 0, 0, 0);
    gl!.disable(gl!.STENCIL_TEST); gl!.depthFunc(gl!.LEQUAL); gl!.depthMask(true);
  }
  /** Entry: scissor disabled, color writes enabled and, if pooled, pool texture bound on unit 1. Exit: default framebuffer with depth writes enabled. */
  function drawWorld(lighting: Lighting, pooled: boolean, showBuildGrid: boolean): void {
    // Tile edges are slightly translucent and have always shown a dark backdrop through them, which draws the faint grid.
    const backdrop = gradeColor([0, 0, 0], lighting);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
    gl!.viewport(0, 0, canvas.width, canvas.height); gl!.depthMask(true); gl!.depthFunc(gl!.LEQUAL);
    gl!.clearColor(backdrop[0], backdrop[1], backdrop[2], 0); gl!.clearDepth(1); gl!.clearStencil(0); gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT | gl!.STENCIL_BUFFER_BIT);
    gl!.enable(gl!.BLEND); gl!.blendEquation(gl!.FUNC_ADD); gl!.blendFuncSeparate(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA, gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
    useWorldProgram(false, false, true, 0, pooled); setLighting(program, lighting);
    gl!.enable(gl!.DEPTH_TEST);
    drawBatches(groundBatches);
    if (buriedTraceBatches.length) drawBuriedTrace(lighting);
    gl!.depthMask(false); drawBatches(decalBatches); gl!.depthMask(true);
    if (showBuildGrid) {
      gl!.disable(gl!.DEPTH_TEST);
      useWorldProgram(false, true, false);
      drawBatches(gridBatches);
      gl!.enable(gl!.DEPTH_TEST);
      useWorldProgram(false, false, true, 0, pooled);
      setLighting(program, lighting);
    }
    drawBatches(standingBatches);
  }
  /** Entry: world color and depth are in the default framebuffer; world lighting uniforms are set. Exit: default framebuffer, depth test disabled and standard alpha blending. */
  function drawAtmosphere(): void {
    // The sky fills only what the world left at the far plane, so nothing behind the park shows through it.
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null); gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.enable(gl!.DEPTH_TEST); gl!.depthFunc(gl!.LEQUAL); gl!.depthMask(false);
    gl!.enable(gl!.BLEND); gl!.blendEquation(gl!.FUNC_ADD); gl!.blendFuncSeparate(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA, gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
    if (atmosphere) drawSky(atmosphere);
    useWorldProgram(false, true, true, 1); drawBatches(skyBatches);
    gl!.depthMask(true);
    gl!.disable(gl!.DEPTH_TEST);
    useWorldProgram(false, true, true); drawBatches(overlayBatches);
    gl!.blendFunc(gl!.SRC_ALPHA, gl!.ONE); useWorldProgram(false, true, false); drawBatches(sparkBatches);
    drawGlows();
    gl!.blendFuncSeparate(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA, gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
  }
  function draw(showBuildGrid: boolean): void {
    gl!.disable(gl!.SCISSOR_TEST); gl!.disable(gl!.CULL_FACE); gl!.disable(gl!.DITHER);
    gl!.colorMask(true, true, true, true);
    const pooled = drawPools();
    drawWorld(atmosphere?.lighting ?? NOON_LIGHTING, pooled, showBuildGrid);
    drawAtmosphere();
  }
  function drawSky({ lighting, horizonY, paper }: Atmosphere): void {
    gl!.disable(gl!.BLEND);
    gl!.useProgram(skyProgram); gl!.bindVertexArray(emptyVao);
    gl!.uniform2f(uniform(skyProgram, "resolution"), canvas.width, canvas.height); gl!.uniform1f(uniform(skyProgram, "horizonY"), horizonY);
    gl!.uniform3fv(uniform(skyProgram, "skyTop"), lighting.skyTop); gl!.uniform3fv(uniform(skyProgram, "skyHorizon"), lighting.skyHorizon);
    gl!.uniform3fv(uniform(skyProgram, "paperVoid"), paper); gl!.uniform1f(uniform(skyProgram, "stars"), lighting.stars);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);
    gl!.enable(gl!.BLEND);
  }
  return {
    render({ world, camera, waterFrame = 0, geometryPasses = [], atmospherePass, showBuildGrid = false, cutaway: requestedCutaway }) {
      if (requestedCutaway && world.undergroundLevel !== undefined) throw new Error("The cutaway replaces the underground cut; use one at a time");
      cutaway = requestedCutaway;
      if (gl!.isContextLost()) return;
      if (canvas.width < 1 || canvas.height < 1) throw new Error("Canvas dimensions must be positive");
      // The camera is in backing-store pixels, so its zoom already includes the device pixel ratio.
      const desiredScale = camera.zoom;
      const scales = Object.keys(atlas.pages).map(Number).sort((a, b) => a - b);
      activeScale = String(scales.find((scale) => scale >= desiredScale) ?? scales[scales.length - 1]);
      pixelRatio = canvas.width / Math.max(1, canvas.clientWidth || canvas.width);
      const viewport = { width: canvas.width, height: canvas.height };
      scene = buildScene(world, camera, viewport, atlas, activeScale, waterFrame);
      uncutSurfaceScene = { triangles: scene.triangles.filter(({ kind }) => kind === "terrain" || kind === "carved-floor" || kind === "path") };
      if (cutaway) {
        for (const triangle of scene.triangles) triangle.cut = CutSide.outside;
        const cut = buildScene({ ...world, undergroundLevel: cutaway.level }, camera, viewport, atlas, activeScale, waterFrame, cutawayBounds(cutaway));
        // Triangles come in pairs that `prepare` packs into one quad, so keep or drop both together.
        for (let index = 0; index < cut.triangles.length; index += 2) {
          const pair = [cut.triangles[index], cut.triangles[index + 1]];
          if (!pair.some((triangle) => polygonTouchesCutaway(triangle.vertices, cutaway!))) continue;
          for (const triangle of pair) { triangle.cut = CutSide.inside; scene.triangles.push(triangle); }
        }
      }
      atmosphere = atmospherePass?.(camera, viewport, atlas, activeScale);
      const clip = (geometry: readonly GeometryQuad[]) => world.undergroundLevel === undefined ? geometry : clipGeometryAboveLevel(geometry, world.undergroundLevel, UNDERGROUND_CONTENT_DEPTH_BIAS);
      const nightLights = atmosphere && world.undergroundLevel !== undefined
        ? clipNightLightsAboveLevel({ bulbs: [...atmosphere.lights], pools: [...atmosphere.pools], glows: [...atmosphere.glows] }, world.undergroundLevel, UNDERGROUND_CONTENT_DEPTH_BIAS)
        : undefined;
      const appendCut = (quads: readonly GeometryQuad[]): void => {
        if (!cutaway) { appendGeometry(scene, clip(quads), viewport); return; }
        const outsideStart = scene.triangles.length; appendGeometry(scene, quads, viewport);
        for (let index = outsideStart; index < scene.triangles.length; index++) scene.triangles[index].cut = CutSide.outside;
        const inWindow = quads.filter((quad) => polygonTouchesCutaway(quad.vertices, cutaway!));
        const insideStart = scene.triangles.length; appendGeometry(scene, clipGeometryAboveLevel(inWindow, cutaway.level, UNDERGROUND_CONTENT_DEPTH_BIAS), viewport);
        for (let index = insideStart; index < scene.triangles.length; index++) scene.triangles[index].cut = CutSide.inside;
      };
      for (const geometryPass of geometryPasses) appendCut(geometryPass(camera, viewport, atlas, activeScale));
      if (nightLights) appendGeometry(scene, nightLights.bulbs, viewport);
      else appendCut(atmosphere?.lights ?? []);
      if (scene.triangles.length / 2 >= 0xffffff) throw new Error("Too many visible objects for picking");
      surfaceScene = { triangles: scene.triangles.filter(({ kind }) => kind === "terrain" || kind === "carved-floor" || kind === "path") };
      facilityScene = { triangles: scene.triangles.filter(({ kind, id }) => kind === "terrain" || kind === "carved-floor" || kind === "path" || kind === "facility" || (kind === "sprite" && id?.startsWith("facility-"))) };
      prepare();
      const gridLighting = atmosphere?.lighting ?? NOON_LIGHTING;
      const gridInk: readonly [number, number, number, number] = gridLighting.lightsOn > 0.5
        ? [0.92, 0.83, 0.63, 0.3]
        : [0.12, 0.08, 0.2, 0.28];
      gridBatches = showBuildGrid ? encode(buildGridQuads(scene.triangles.filter((triangle) => triangle.cut !== CutSide.inside), gridInk).map((quad) => fromGeometry(quad))) : [];
      skyBatches = encode((atmosphere?.sky ?? []).map((quad) => fromGeometry(quad)));
      decalBatches = encode(clip(atmosphere?.decals ?? []).map((quad) => fromGeometry(quad, cutaway ? CutSide.outside : CutSide.everywhere)));
      poolBatches = encode((nightLights?.pools ?? atmosphere?.pools ?? []).map((quad) => fromGeometry(quad)));
      overlayBatches = encode(clip(atmosphere?.overlay ?? []).map((quad) => fromGeometry(quad)));
      sparkBatches = encode(clip(atmosphere?.sparks ?? []).map((quad) => fromGeometry(quad)));
      glowBatches = encode((nightLights?.glows ?? atmosphere?.glows ?? []).map((glow) => ({
        vertices: ([[-1, -1], [1, -1], [1, 1], [-1, 1]] as const).map(([across, down]) => ({ x: glow.x + across * glow.radius, y: glow.y + down * glow.radius, depth: 0, height: 0, u: (across + 1) / 2, v: (down + 1) / 2 })),
        page: SHAPE_PAGES.glow, color: [...glow.color, glow.intensity] as const, light: 1, pickId: 0, cut: CutSide.everywhere,
      })));
      if (pickWidth !== canvas.width || pickHeight !== canvas.height) {
        pickWidth = canvas.width; pickHeight = canvas.height;
        gl!.activeTexture(gl!.TEXTURE0);
        gl!.bindTexture(gl!.TEXTURE_2D, pickTexture); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.NEAREST); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.NEAREST);
        gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA8, pickWidth, pickHeight, 0, gl!.RGBA, gl!.UNSIGNED_BYTE, null);
        gl!.bindRenderbuffer(gl!.RENDERBUFFER, depthBuffer); gl!.renderbufferStorage(gl!.RENDERBUFFER, gl!.DEPTH_COMPONENT24, pickWidth, pickHeight);
        gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer); gl!.framebufferTexture2D(gl!.FRAMEBUFFER, gl!.COLOR_ATTACHMENT0, gl!.TEXTURE_2D, pickTexture, 0);
        gl!.framebufferRenderbuffer(gl!.FRAMEBUFFER, gl!.DEPTH_ATTACHMENT, gl!.RENDERBUFFER, depthBuffer);
        if (gl!.checkFramebufferStatus(gl!.FRAMEBUFFER) !== gl!.FRAMEBUFFER_COMPLETE) throw new Error("Picking framebuffer is incomplete");
      }
      draw(showBuildGrid);
      pickingDirty = true;
    },
    isContextAvailable() { return !gl!.isContextLost(); },
    pick(x, y) {
      if (x < 0 || y < 0 || x >= pickWidth || y >= pickHeight) return null;
      ensurePicking();
      const pixel = new Uint8Array(4); gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer);
      gl!.readPixels(Math.floor(x), pickHeight - 1 - Math.floor(y), 1, 1, gl!.RGBA, gl!.UNSIGNED_BYTE, pixel); gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
      return resolvePick(pixel, 0, Math.floor(x), Math.floor(y));
    },
    pickRegion(x, y, width, height) {
      validatePickRegion(x, y, width, height);
      const result: (Pick | null)[] = Array(width * height).fill(null);
      const left = Math.max(0, x); const top = Math.max(0, y);
      const right = Math.min(pickWidth, x + width); const bottom = Math.min(pickHeight, y + height);
      if (left >= right || top >= bottom) return result;
      const clippedWidth = right - left; const clippedHeight = bottom - top;
      const pixels = new Uint8Array(clippedWidth * clippedHeight * 4);
      ensurePicking();
      gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer);
      gl!.readPixels(left, pickHeight - bottom, clippedWidth, clippedHeight, gl!.RGBA, gl!.UNSIGNED_BYTE, pixels);
      gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
      for (let row = 0; row < clippedHeight; row++) for (let column = 0; column < clippedWidth; column++) {
        const pixelOffset = ((clippedHeight - 1 - row) * clippedWidth + column) * 4;
        result[(top - y + row) * width + left - x + column] = resolvePick(pixels, pixelOffset, left + column, top + row);
      }
      return result;
    },
    pickSurface(x, y) { return pickVisible(surfaceScene, x, y); },
    pickSurfaceUncut(x, y) { return pickScene(uncutSurfaceScene, Math.floor(x) + 0.5, Math.floor(y) + 0.5, 0.125); },
    pickFacilityPlacement(x, y) { return pickVisible(facilityScene, x, y); },
    waitForGpu() { gl!.readPixels(0, 0, 1, 1, gl!.RGBA, gl!.UNSIGNED_BYTE, new Uint8Array(4)); },
  };
}

export function createWorldRenderer(canvas: HTMLCanvasElement, atlas: Atlas, images: Record<string, HTMLImageElement[]>, preserveDrawingBuffer = false): WorldRenderer {
  let contextLost = false;
  let renderer = createWorldRendererInstance(canvas, atlas, images, preserveDrawingBuffer);
  canvas.addEventListener("webglcontextlost", (event) => {
    event.preventDefault();
    contextLost = true;
  });
  canvas.addEventListener("webglcontextrestored", () => {
    renderer = createWorldRendererInstance(canvas, atlas, images, preserveDrawingBuffer);
    contextLost = false;
  });
  return {
    render(input) {
      if (contextLost) return;
      renderer.render(input);
    },
    isContextAvailable() { return !contextLost && renderer.isContextAvailable(); },
    pick(x, y) { return contextLost ? null : renderer.pick(x, y); },
    pickRegion(x, y, width, height) {
      if (!contextLost) return renderer.pickRegion(x, y, width, height);
      validatePickRegion(x, y, width, height);
      return Array(width * height).fill(null);
    },
    pickSurface(x, y) { return renderer.pickSurface(x, y); },
    pickSurfaceUncut(x, y) { return contextLost ? null : renderer.pickSurfaceUncut(x, y); },
    pickFacilityPlacement(x, y) { return contextLost ? null : renderer.pickFacilityPlacement(x, y); },
    waitForGpu() { if (!contextLost) renderer.waitForGpu(); },
  };
}
