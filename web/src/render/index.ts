import type { Camera, Viewport } from "../camera";
import { gradeColor, GRADE_GLSL, hexColor, lightingAt, NOON, type Lighting } from "./daylight";
import type { Glow } from "./effects";
import { appendGeometry, buildScene, clipGeometryAboveLevel, pickScene, SHAPE_PAGES, UNDERGROUND_CONTENT_DEPTH_BIAS, UNPICKABLE_KINDS, type Atlas, type GeometryQuad, type Pick, type Scene, type Triangle, type Vertex, type WorldView } from "./scene";
export type { Atlas, GeometryQuad, Pick, WorldSprite, WorldView } from "./scene";
export { terrainHeightAt } from "./scene";
export { clipGeometryAboveLevel } from "./scene";
const PAPER = hexColor("#f2eadb");
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
  render(world: WorldView, camera: Camera, waterFrame?: number, geometryPasses?: readonly GeometryPass[], atmospherePass?: AtmospherePass): void;
  pick(x: number, y: number): Pick | null;
  pickSurface(x: number, y: number): Pick | null;
  waitForGpu(): void;
}

export type GeometryPass = (camera: Camera, viewport: Viewport, atlas: Atlas, scale: string) => readonly GeometryQuad[];

/** Everything the time of day adds around the world. Quads in `sky`, `overlay` and `sparks` are in canvas pixels and are not depth tested. */
export interface Atmosphere {
  lighting: Lighting;
  /** Where the sky gives way to paper below the park, in canvas pixels from the top. */
  horizonY: number;
  /** Sun, moon and clouds, behind the world. */
  sky: readonly GeometryQuad[];
  /** Ground shadows, over terrain and paths and under everything standing on them. */
  decals: readonly GeometryQuad[];
  /** Depth-tested world geometry that is never picked, such as bulbs. */
  lights: readonly GeometryQuad[];
  /** Confetti, blended over the world. */
  overlay: readonly GeometryQuad[];
  /** Firework sparks, added to the world. */
  sparks: readonly GeometryQuad[];
  glows: readonly Glow[];
}
export type AtmospherePass = (camera: Camera, viewport: Viewport, atlas: Atlas, scale: string) => Atmosphere;

/** Surfaces that stay partly lit at night, as if by path lamps, so a busy park stays readable. */
const PATH_NIGHT_LIGHT = 0.4;
const GUEST_NIGHT_LIGHT = 0.45;
const GROUND_KINDS: ReadonlySet<string> = new Set(["terrain", "cliff", "water", "path", "carved-floor", "cavern-wall"]);
const GLOW_DOWNSCALE = 4;
/** The glow layer is printed slightly out of register with the world, in CSS pixels. */
const GLOW_MISREGISTRATION = { x: 2, y: -1.5 };
const FLOATS_PER_QUAD = 28;
const shapeCodes: Record<number, number> = { [SHAPE_PAGES.flat]: 0, [SHAPE_PAGES.glow]: 1, [SHAPE_PAGES.disc]: 2, [SHAPE_PAGES.softDisc]: 3 };

interface Batch { page: number; data: Float32Array; count: number }
interface QuadSource { vertices: readonly Vertex[]; page: number; color: Triangle["color"]; light: number; pickId: number }

function nightLight(triangle: Triangle): number {
  if (triangle.light !== undefined) return triangle.light;
  if (triangle.kind === "path") return PATH_NIGHT_LIGHT;
  if (triangle.kind === "rider" || triangle.id?.startsWith("guest-")) return GUEST_NIGHT_LIGHT;
  return 0;
}

/** Packs quads into instance data, one batch per run of quads that share an atlas page or shape. */
function encode(quads: readonly QuadSource[]): Batch[] {
  const batches: Batch[] = [];
  let start = 0;
  while (start < quads.length) {
    const page = quads[start].page; let end = start + 1;
    while (end < quads.length && quads[end].page === page) end++;
    const data = new Float32Array((end - start) * FLOATS_PER_QUAD);
    for (let i = start; i < end; i++) {
      const { vertices, color, light, pickId } = quads[i]; const offset = (i - start) * FLOATS_PER_QUAD;
      vertices.forEach((vertex, j) => { data.set([vertex.x, vertex.y, vertex.depth, vertex.u], offset + j * 4); data[offset + 16 + j] = vertex.v; });
      data.set([(pickId & 255) / 255, ((pickId >> 8) & 255) / 255, ((pickId >> 16) & 255) / 255], offset + 20);
      data.set(color, offset + 23);
      data[offset + 27] = light;
    }
    batches.push({ page, data, count: end - start }); start = end;
  }
  return batches;
}

const fromGeometry = (quad: GeometryQuad): QuadSource => ({ vertices: quad.vertices, page: quad.page ?? SHAPE_PAGES.flat, color: quad.color, light: quad.light ?? 0, pickId: 0 });

export function createWorldRenderer(canvas: HTMLCanvasElement, atlas: Atlas, images: Record<string, HTMLImageElement[]>): WorldRenderer {
  const gl = canvas.getContext("webgl2", { alpha: false });
  if (!gl) throw new Error("WebGL2 is unavailable");
  for (const [scale, pages] of Object.entries(atlas.pages)) if (images[scale]?.length !== pages.length) throw new Error("Atlas page count does not match images");
  function shader(type: number, source: string): WebGLShader {
    const result = gl!.createShader(type);
    if (!result) throw new Error("Could not create shader");
    gl!.shaderSource(result, source); gl!.compileShader(result);
    if (!gl!.getShaderParameter(result, gl!.COMPILE_STATUS)) throw new Error(`Shader compilation failed: ${gl!.getShaderInfoLog(result)}`);
    return result;
  }
  function link(vertexSource: string, fragmentSource: string): WebGLProgram {
    const vertex = shader(gl!.VERTEX_SHADER, vertexSource); const fragment = shader(gl!.FRAGMENT_SHADER, fragmentSource);
    const result = gl!.createProgram();
    if (!result) throw new Error("Could not create program");
    gl!.attachShader(result, vertex); gl!.attachShader(result, fragment); gl!.linkProgram(result);
    if (!gl!.getProgramParameter(result, gl!.LINK_STATUS)) throw new Error(`Program link failed: ${gl!.getProgramInfoLog(result)}`);
    gl!.deleteShader(vertex); gl!.deleteShader(fragment);
    return result;
  }
  const program = link(`#version 300 es
    precision highp float;
    layout(location=0) in vec4 a; layout(location=1) in vec4 b;
    layout(location=2) in vec4 c; layout(location=3) in vec4 d;
    layout(location=4) in vec4 vs; layout(location=5) in vec3 objectId; layout(location=6) in vec4 tint; layout(location=7) in float light;
    uniform vec2 resolution; uniform vec2 depthRange; uniform bool overlay; uniform float overlayDepth;
    out vec2 uv; flat out vec3 id; flat out vec4 fill; flat out float lit;
    void main() {
      int indices[6] = int[6](0,1,2,0,2,3); int i = indices[gl_VertexID];
      vec4 p = i == 0 ? a : i == 1 ? b : i == 2 ? c : d;
      float v = i == 0 ? vs.x : i == 1 ? vs.y : i == 2 ? vs.z : vs.w;
      vec2 screen = p.xy / resolution;
      float z = overlay ? overlayDepth : 1.0 - (p.z - depthRange.x) / depthRange.y * 2.0;
      gl_Position = vec4(screen.x*2.0-1.0, 1.0-screen.y*2.0, z, 1.0);
      uv = vec2(p.w,v); id = objectId; fill = tint; lit = light;
    }`, `#version 300 es
    precision highp float; uniform sampler2D atlas; uniform bool picking; uniform bool textured; uniform int shape; uniform bool graded;
    ${GRADE_GLSL}
    in vec2 uv; flat in vec3 id; flat in vec4 fill; flat in float lit; out vec4 color;
    void main() {
      vec4 texel = textured ? texture(atlas,uv) * fill : fill;
      if (shape != 0) {
        float r = length(uv * 2.0 - 1.0);
        texel.a *= shape == 1 ? pow(max(0.0, 1.0 - r * r), 2.0) : shape == 2 ? 1.0 - smoothstep(0.75, 1.0, r) : 1.0 - smoothstep(0.35, 1.0, r);
      }
      if (texel.a < (shape != 0 ? 0.003 : 0.05) || (picking && all(equal(id, vec3(0.0))))) discard;
      color = picking ? vec4(id,1.0) : vec4(graded ? grade(texel.rgb, lit) : texel.rgb, texel.a);
    }`);
  const fullScreenVertex = (depth: number) => `#version 300 es
    void main() { vec2 corner = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); gl_Position = vec4(corner * 2.0 - 1.0, ${depth.toFixed(1)}, 1.0); }`;
  const skyProgram = link(fullScreenVertex(1), `#version 300 es
    precision highp float;
    uniform vec2 resolution; uniform float horizonY; uniform vec3 skyTop; uniform vec3 skyHorizon; uniform vec3 paperVoid; uniform float stars;
    out vec4 color;
    float hash(vec2 point) { return fract(sin(dot(point, vec2(127.1, 311.7))) * 43758.5453); }
    void main() {
      float y = resolution.y - gl_FragCoord.y;
      vec3 sky = mix(skyTop, skyHorizon, pow(clamp(y / clamp(horizonY, 1.0, resolution.y), 0.0, 1.0), 1.6));
      float band = resolution.y * 0.06;
      vec2 cell = floor(gl_FragCoord.xy / 22.0);
      vec2 star = (cell + vec2(hash(cell + 1.3), hash(cell + 2.7))) * 22.0;
      float twinkle = step(hash(cell), 0.08) * (1.0 - smoothstep(0.6, 1.6, length(gl_FragCoord.xy - star))) * (0.45 + 0.55 * hash(cell + 5.1));
      sky = mix(sky, vec3(0.97, 0.93, 0.85), stars * twinkle * (1.0 - smoothstep(horizonY - band * 2.0, horizonY - band, y)));
      vec3 result = mix(sky, paperVoid, smoothstep(horizonY - band * 0.3, horizonY + band, y));
      color = vec4(result * (0.985 + 0.03 * hash(floor(gl_FragCoord.xy))), 1.0);
    }`);
  const glowProgram = link(fullScreenVertex(0), `#version 300 es
    precision highp float;
    uniform sampler2D glow; uniform vec2 resolution; uniform vec2 offset;
    out vec4 color;
    void main() {
      vec3 light = texture(glow, (gl_FragCoord.xy - offset) / resolution).rgb;
      float halftone = 1.0 - smoothstep(0.6, 1.6, length(mod(gl_FragCoord.xy, 3.0) - 1.5));
      color = vec4(light * (0.8 + 0.35 * halftone), 1.0);
    }`);
  const vao = gl.createVertexArray(); const emptyVao = gl.createVertexArray(); const buffer = gl.createBuffer(); const framebuffer = gl.createFramebuffer();
  const pickTexture = gl.createTexture(); const depthBuffer = gl.createRenderbuffer();
  const glowFramebuffer = gl.createFramebuffer(); const glowTexture = gl.createTexture();
  if (!vao || !emptyVao || !buffer || !framebuffer || !pickTexture || !depthBuffer || !glowFramebuffer || !glowTexture) throw new Error("Could not create WebGL resources");
  gl.bindVertexArray(vao); gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const stride = FLOATS_PER_QUAD * 4;
  for (let i = 0; i < 4; i++) { gl.enableVertexAttribArray(i); gl.vertexAttribPointer(i, 4, gl.FLOAT, false, stride, i * 16); gl.vertexAttribDivisor(i, 1); }
  gl.enableVertexAttribArray(4); gl.vertexAttribPointer(4, 4, gl.FLOAT, false, stride, 64); gl.vertexAttribDivisor(4, 1);
  gl.enableVertexAttribArray(5); gl.vertexAttribPointer(5, 3, gl.FLOAT, false, stride, 80); gl.vertexAttribDivisor(5, 1);
  gl.enableVertexAttribArray(6); gl.vertexAttribPointer(6, 4, gl.FLOAT, false, stride, 92); gl.vertexAttribDivisor(6, 1);
  gl.enableVertexAttribArray(7); gl.vertexAttribPointer(7, 1, gl.FLOAT, false, stride, 108); gl.vertexAttribDivisor(7, 1);
  const textures = Object.fromEntries(Object.entries(images).map(([scale, pageImages]) => [scale, pageImages.map((image) => {
    const texture = gl!.createTexture(); if (!texture) throw new Error("Could not create atlas texture");
    gl!.bindTexture(gl!.TEXTURE_2D, texture);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.LINEAR); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.LINEAR);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_S, gl!.CLAMP_TO_EDGE); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_T, gl!.CLAMP_TO_EDGE);
    gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA, gl!.RGBA, gl!.UNSIGNED_BYTE, image); return texture;
  })]));
  let activeScale = "1";
  gl.useProgram(program); gl.uniform1i(gl.getUniformLocation(program, "atlas"), 0); gl.disable(gl.DITHER); gl.depthFunc(gl.LEQUAL);
  const uniform = (target: WebGLProgram, name: string): WebGLUniformLocation | null => gl!.getUniformLocation(target, name);
  let scene: Scene = { triangles: [] };
  let surfaceScene: Scene = { triangles: [] };
  let groundBatches: Batch[] = []; let standingBatches: Batch[] = [];
  let skyBatches: Batch[] = []; let decalBatches: Batch[] = []; let overlayBatches: Batch[] = []; let sparkBatches: Batch[] = []; let glowBatches: Batch[] = [];
  let atmosphere: Atmosphere | undefined;
  let depthMinimum = 0; let depthSpan = 2;
  let pickWidth = 0; let pickHeight = 0;
  let glowWidth = 0; let glowHeight = 0;
  let pixelRatio = 1;
  function prepare(): void {
    let minimum = Infinity; let maximum = -Infinity;
    for (const triangle of scene.triangles) for (const vertex of triangle.vertices) { minimum = Math.min(minimum, vertex.depth); maximum = Math.max(maximum, vertex.depth); }
    depthMinimum = minimum - 1; depthSpan = Math.max(2, maximum - minimum + 2);
    const ground: QuadSource[] = []; const standing: QuadSource[] = [];
    for (let i = 0; i < scene.triangles.length / 2; i++) {
      const first = scene.triangles[i * 2]; const second = scene.triangles[i * 2 + 1];
      const quad = { vertices: [...first.vertices, second.vertices[2]], page: first.page, color: first.color, light: nightLight(first), pickId: UNPICKABLE_KINDS.has(first.kind) ? 0 : i + 1 };
      (GROUND_KINDS.has(first.kind) ? ground : standing).push(quad);
    }
    groundBatches = encode(ground); standingBatches = encode(standing);
  }
  function setLighting(target: WebGLProgram, lighting: Lighting): void {
    gl!.uniform3fv(uniform(target, "gradeMultiply"), lighting.multiply); gl!.uniform3fv(uniform(target, "gradeShadowInk"), lighting.shadowInk);
    gl!.uniform3fv(uniform(target, "gradeHighlightInk"), lighting.highlightInk); gl!.uniform1f(uniform(target, "gradeInkAmount"), lighting.inkAmount);
    gl!.uniform1f(uniform(target, "gradeLightsOn"), lighting.lightsOn);
  }
  function drawBatches(batches: readonly Batch[]): void {
    for (const { page, data, count } of batches) {
      gl!.uniform1i(uniform(program, "textured"), page >= 0 ? 1 : 0);
      gl!.uniform1i(uniform(program, "shape"), page >= 0 ? 0 : shapeCodes[page]);
      gl!.bindTexture(gl!.TEXTURE_2D, page >= 0 ? textures[activeScale][page] : null); gl!.bufferData(gl!.ARRAY_BUFFER, data, gl!.DYNAMIC_DRAW);
      gl!.drawArraysInstanced(gl!.TRIANGLES, 0, 6, count);
    }
  }
  /** Overlays are drawn flat at `overlayDepth` in clip space: 0 to sit over everything, 1 to fill only what the world left empty. */
  function useWorldProgram(picking: boolean, overlay: boolean, graded: boolean, overlayDepth = 0): void {
    gl!.useProgram(program); gl!.uniform1i(uniform(program, "picking"), picking ? 1 : 0);
    gl!.uniform1i(uniform(program, "overlay"), overlay ? 1 : 0); gl!.uniform1f(uniform(program, "overlayDepth"), overlayDepth); gl!.uniform1i(uniform(program, "graded"), graded ? 1 : 0);
    gl!.uniform2f(uniform(program, "resolution"), canvas.width, canvas.height);
    gl!.uniform2f(uniform(program, "depthRange"), depthMinimum, depthSpan);
    gl!.bindVertexArray(vao); gl!.bindBuffer(gl!.ARRAY_BUFFER, buffer); gl!.activeTexture(gl!.TEXTURE0);
  }
  function drawPicking(): void {
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer);
    gl!.viewport(0, 0, canvas.width, canvas.height); gl!.clearColor(0, 0, 0, 0); gl!.clearDepth(1); gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT);
    gl!.disable(gl!.BLEND); gl!.enable(gl!.DEPTH_TEST); gl!.depthMask(true);
    useWorldProgram(true, false, false);
    drawBatches(groundBatches); drawBatches(standingBatches);
  }
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
    gl!.blendFunc(gl!.SRC_ALPHA, gl!.ONE);
    useWorldProgram(false, true, false); drawBatches(glowBatches);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null); gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.useProgram(glowProgram); gl!.bindVertexArray(emptyVao);
    gl!.activeTexture(gl!.TEXTURE0); gl!.bindTexture(gl!.TEXTURE_2D, glowTexture); gl!.uniform1i(uniform(glowProgram, "glow"), 0);
    gl!.uniform2f(uniform(glowProgram, "resolution"), canvas.width, canvas.height);
    gl!.uniform2f(uniform(glowProgram, "offset"), GLOW_MISREGISTRATION.x * pixelRatio, -GLOW_MISREGISTRATION.y * pixelRatio);
    gl!.blendFunc(gl!.ONE, gl!.ONE);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);
  }
  function draw(): void {
    const lighting = atmosphere?.lighting ?? NOON_LIGHTING;
    // Tile edges are slightly translucent and have always shown a dark backdrop through them, which draws the faint grid.
    const backdrop = gradeColor([0, 0, 0], lighting);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
    gl!.viewport(0, 0, canvas.width, canvas.height); gl!.clearColor(backdrop[0], backdrop[1], backdrop[2], 0); gl!.clearDepth(1); gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT);
    gl!.enable(gl!.BLEND); gl!.blendFuncSeparate(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA, gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
    useWorldProgram(false, false, true); setLighting(program, lighting);
    gl!.enable(gl!.DEPTH_TEST);
    drawBatches(groundBatches);
    gl!.depthMask(false); drawBatches(decalBatches); gl!.depthMask(true);
    drawBatches(standingBatches);
    // The sky fills only what the world left at the far plane, so nothing behind the park shows through it.
    gl!.depthMask(false);
    if (atmosphere) drawSky(atmosphere);
    useWorldProgram(false, true, true, 1); drawBatches(skyBatches);
    gl!.depthMask(true);
    gl!.disable(gl!.DEPTH_TEST);
    useWorldProgram(false, true, true); drawBatches(overlayBatches);
    gl!.blendFunc(gl!.SRC_ALPHA, gl!.ONE); useWorldProgram(false, true, false); drawBatches(sparkBatches);
    drawGlows();
    gl!.blendFuncSeparate(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA, gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
  }
  function drawSky({ lighting, horizonY }: Atmosphere): void {
    gl!.disable(gl!.BLEND);
    gl!.useProgram(skyProgram); gl!.bindVertexArray(emptyVao);
    gl!.uniform2f(uniform(skyProgram, "resolution"), canvas.width, canvas.height); gl!.uniform1f(uniform(skyProgram, "horizonY"), horizonY);
    gl!.uniform3fv(uniform(skyProgram, "skyTop"), lighting.skyTop); gl!.uniform3fv(uniform(skyProgram, "skyHorizon"), lighting.skyHorizon);
    gl!.uniform3fv(uniform(skyProgram, "paperVoid"), gradeColor(PAPER, lighting)); gl!.uniform1f(uniform(skyProgram, "stars"), lighting.stars);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);
    gl!.enable(gl!.BLEND);
  }
  return {
    render(world, camera, waterFrame = 0, geometryPasses = [], atmospherePass) {
      if (canvas.width < 1 || canvas.height < 1) throw new Error("Canvas dimensions must be positive");
      // The camera is in backing-store pixels, so its zoom already includes the device pixel ratio.
      const desiredScale = camera.zoom;
      const scales = Object.keys(atlas.pages).map(Number).sort((a, b) => a - b);
      activeScale = String(scales.find((scale) => scale >= desiredScale) ?? scales[scales.length - 1]);
      pixelRatio = canvas.width / Math.max(1, canvas.clientWidth || canvas.width);
      const viewport = { width: canvas.width, height: canvas.height };
      scene = buildScene(world, camera, viewport, atlas, activeScale, waterFrame);
      atmosphere = atmospherePass?.(camera, viewport, atlas, activeScale);
      const clip = (geometry: readonly GeometryQuad[]) => world.undergroundLevel === undefined ? geometry : clipGeometryAboveLevel(geometry, world.undergroundLevel, UNDERGROUND_CONTENT_DEPTH_BIAS);
      for (const geometryPass of geometryPasses) appendGeometry(scene, clip(geometryPass(camera, viewport, atlas, activeScale)), viewport);
      appendGeometry(scene, clip(atmosphere?.lights ?? []), viewport);
      if (scene.triangles.length / 2 >= 0xffffff) throw new Error("Too many visible objects for picking");
      surfaceScene = { triangles: scene.triangles.filter(({ kind }) => kind === "terrain" || kind === "carved-floor" || kind === "path") };
      prepare();
      skyBatches = encode((atmosphere?.sky ?? []).map(fromGeometry));
      decalBatches = encode(clip(atmosphere?.decals ?? []).map(fromGeometry));
      overlayBatches = encode((atmosphere?.overlay ?? []).map(fromGeometry));
      sparkBatches = encode((atmosphere?.sparks ?? []).map(fromGeometry));
      glowBatches = encode((atmosphere?.glows ?? []).map((glow) => ({
        vertices: ([[-1, -1], [1, -1], [1, 1], [-1, 1]] as const).map(([across, down]) => ({ x: glow.x + across * glow.radius, y: glow.y + down * glow.radius, depth: 0, height: 0, u: (across + 1) / 2, v: (down + 1) / 2 })),
        page: SHAPE_PAGES.glow, color: [...glow.color, glow.intensity] as const, light: 1, pickId: 0,
      })));
      if (pickWidth !== canvas.width || pickHeight !== canvas.height) {
        pickWidth = canvas.width; pickHeight = canvas.height;
        gl!.bindTexture(gl!.TEXTURE_2D, pickTexture); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.NEAREST); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.NEAREST);
        gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA8, pickWidth, pickHeight, 0, gl!.RGBA, gl!.UNSIGNED_BYTE, null);
        gl!.bindRenderbuffer(gl!.RENDERBUFFER, depthBuffer); gl!.renderbufferStorage(gl!.RENDERBUFFER, gl!.DEPTH_COMPONENT24, pickWidth, pickHeight);
        gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer); gl!.framebufferTexture2D(gl!.FRAMEBUFFER, gl!.COLOR_ATTACHMENT0, gl!.TEXTURE_2D, pickTexture, 0);
        gl!.framebufferRenderbuffer(gl!.FRAMEBUFFER, gl!.DEPTH_ATTACHMENT, gl!.RENDERBUFFER, depthBuffer);
        if (gl!.checkFramebufferStatus(gl!.FRAMEBUFFER) !== gl!.FRAMEBUFFER_COMPLETE) throw new Error("Picking framebuffer is incomplete");
      }
      draw(); drawPicking(); gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
    },
    pick(x, y) {
      if (x < 0 || y < 0 || x >= pickWidth || y >= pickHeight) return null;
      const pixel = new Uint8Array(4); gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer);
      gl!.readPixels(Math.floor(x), pickHeight - 1 - Math.floor(y), 1, 1, gl!.RGBA, gl!.UNSIGNED_BYTE, pixel); gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
      const id = pixel[0] + pixel[1] * 256 + pixel[2] * 65536; if (!id) return null;
      return pickScene({ triangles: scene.triangles.slice((id - 1) * 2, id * 2) }, Math.floor(x) + 0.5, Math.floor(y) + 0.5, 0.125);
    },
    pickSurface(x, y) { return pickScene(surfaceScene, Math.floor(x) + 0.5, Math.floor(y) + 0.5, 0.125); },
    waitForGpu() { gl!.readPixels(0, 0, 1, 1, gl!.RGBA, gl!.UNSIGNED_BYTE, new Uint8Array(4)); },
  };
}
