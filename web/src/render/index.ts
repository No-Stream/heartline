import type { Camera, Viewport } from "../camera";
import { appendGeometry, buildScene, clipGeometryAboveLevel, pickScene, UNDERGROUND_CONTENT_DEPTH_BIAS, type Atlas, type GeometryQuad, type Pick, type Scene, type WorldView } from "./scene";
export type { Atlas, GeometryQuad, Pick, WorldSprite, WorldView } from "./scene";
export { terrainHeightAt } from "./scene";
export { clipGeometryAboveLevel } from "./scene";

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
  render(world: WorldView, camera: Camera, waterFrame?: number, geometryPasses?: readonly GeometryPass[]): void;
  pick(x: number, y: number): Pick | null;
  pickSurface(x: number, y: number): Pick | null;
  waitForGpu(): void;
}

export type GeometryPass = (camera: Camera, viewport: Viewport, atlas: Atlas, scale: string) => readonly GeometryQuad[];

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
  const vertex = shader(gl.VERTEX_SHADER, `#version 300 es
    precision highp float;
    layout(location=0) in vec4 a; layout(location=1) in vec4 b;
    layout(location=2) in vec4 c; layout(location=3) in vec4 d;
    layout(location=4) in vec4 vs; layout(location=5) in vec3 objectId; layout(location=6) in vec4 tint;
    uniform vec2 resolution; uniform vec2 depthRange;
    out vec2 uv; flat out vec3 id; flat out vec4 fill;
    void main() {
      int indices[6] = int[6](0,1,2,0,2,3); int i = indices[gl_VertexID];
      vec4 p = i == 0 ? a : i == 1 ? b : i == 2 ? c : d;
      float v = i == 0 ? vs.x : i == 1 ? vs.y : i == 2 ? vs.z : vs.w;
      vec2 screen = p.xy / resolution;
      float z = 1.0 - (p.z - depthRange.x) / depthRange.y * 2.0;
      gl_Position = vec4(screen.x*2.0-1.0, 1.0-screen.y*2.0, z, 1.0);
      uv = vec2(p.w,v); id = objectId; fill = tint;
    }`);
  const fragment = shader(gl.FRAGMENT_SHADER, `#version 300 es
    precision highp float; uniform sampler2D atlas; uniform bool picking; uniform bool textured;
    in vec2 uv; flat in vec3 id; flat in vec4 fill; out vec4 color;
    void main() { vec4 texel = textured ? texture(atlas,uv) * fill : fill; if (texel.a < 0.05 || (picking && all(equal(id, vec3(0.0))))) discard;
      color = picking ? vec4(id,1.0) : texel; }`);
  const program = gl.createProgram();
  if (!program) throw new Error("Could not create program");
  gl.attachShader(program, vertex); gl.attachShader(program, fragment); gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(`Program link failed: ${gl.getProgramInfoLog(program)}`);
  gl.deleteShader(vertex); gl.deleteShader(fragment);
  const vao = gl.createVertexArray(); const buffer = gl.createBuffer(); const framebuffer = gl.createFramebuffer();
  const pickTexture = gl.createTexture(); const depthBuffer = gl.createRenderbuffer();
  if (!vao || !buffer || !framebuffer || !pickTexture || !depthBuffer) throw new Error("Could not create WebGL resources");
  gl.bindVertexArray(vao); gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const stride = 27 * 4;
  for (let i = 0; i < 4; i++) { gl.enableVertexAttribArray(i); gl.vertexAttribPointer(i, 4, gl.FLOAT, false, stride, i * 16); gl.vertexAttribDivisor(i, 1); }
  gl.enableVertexAttribArray(4); gl.vertexAttribPointer(4, 4, gl.FLOAT, false, stride, 64); gl.vertexAttribDivisor(4, 1);
  gl.enableVertexAttribArray(5); gl.vertexAttribPointer(5, 3, gl.FLOAT, false, stride, 80); gl.vertexAttribDivisor(5, 1);
  gl.enableVertexAttribArray(6); gl.vertexAttribPointer(6, 4, gl.FLOAT, false, stride, 92); gl.vertexAttribDivisor(6, 1);
  const textures = Object.fromEntries(Object.entries(images).map(([scale, pageImages]) => [scale, pageImages.map((image) => {
    const texture = gl!.createTexture(); if (!texture) throw new Error("Could not create atlas texture");
    gl!.bindTexture(gl!.TEXTURE_2D, texture);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.LINEAR); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.LINEAR);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_S, gl!.CLAMP_TO_EDGE); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_T, gl!.CLAMP_TO_EDGE);
    gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA, gl!.RGBA, gl!.UNSIGNED_BYTE, image); return texture;
  })]));
  let activeScale = "1";
  gl.useProgram(program); gl.uniform1i(gl.getUniformLocation(program, "atlas"), 0); gl.enable(gl.DEPTH_TEST); gl.disable(gl.DITHER); gl.depthFunc(gl.LEQUAL);
  let scene: Scene = { triangles: [] };
  let surfaceScene: Scene = { triangles: [] };
  let batches: { page: number; data: Float32Array; count: number }[] = [];
  let depthMinimum = 0; let depthSpan = 2;
  let pickWidth = 0; let pickHeight = 0;
  function prepare(): void {
    let minimum = Infinity; let maximum = -Infinity;
    for (const triangle of scene.triangles) for (const vertex of triangle.vertices) { minimum = Math.min(minimum, vertex.depth); maximum = Math.max(maximum, vertex.depth); }
    depthMinimum = minimum - 1; depthSpan = Math.max(2, maximum - minimum + 2);
    batches = [];
    let start = 0;
    while (start < scene.triangles.length / 2) {
      const page = scene.triangles[start * 2].page; let end = start + 1;
      while (end < scene.triangles.length / 2 && scene.triangles[end * 2].page === page) end++;
      const data = new Float32Array((end - start) * 27);
      for (let i = start; i < end; i++) {
        const first = scene.triangles[i * 2]; const second = scene.triangles[i * 2 + 1];
        const vertices = [...first.vertices, second.vertices[2]]; const offset = (i - start) * 27;
        vertices.forEach((vertex, j) => { data.set([vertex.x, vertex.y, vertex.depth, vertex.u], offset + j * 4); data[offset + 16 + j] = vertex.v; });
        const id = first.kind === "path-preview" ? 0 : i + 1; data.set([(id & 255) / 255, ((id >> 8) & 255) / 255, ((id >> 16) & 255) / 255], offset + 20);
        data.set(first.color, offset + 23);
      }
      batches.push({ page, data, count: end - start }); start = end;
    }
  }
  function draw(picking: boolean): void {
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, picking ? framebuffer : null);
    gl!.viewport(0, 0, canvas.width, canvas.height); gl!.clearColor(0, 0, 0, 0); gl!.clearDepth(1); gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT);
    if (picking) gl!.disable(gl!.BLEND); else { gl!.enable(gl!.BLEND); gl!.blendFuncSeparate(gl!.SRC_ALPHA, gl!.ONE_MINUS_SRC_ALPHA, gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA); }
    gl!.useProgram(program); gl!.uniform1i(gl!.getUniformLocation(program, "picking"), picking ? 1 : 0);
    gl!.uniform2f(gl!.getUniformLocation(program, "resolution"), canvas.width, canvas.height);
    gl!.uniform2f(gl!.getUniformLocation(program, "depthRange"), depthMinimum, depthSpan);
    gl!.bindVertexArray(vao); gl!.bindBuffer(gl!.ARRAY_BUFFER, buffer); gl!.activeTexture(gl!.TEXTURE0);
    for (const { page, data, count } of batches) {
      gl!.uniform1i(gl!.getUniformLocation(program, "textured"), page >= 0 ? 1 : 0);
      gl!.bindTexture(gl!.TEXTURE_2D, page >= 0 ? textures[activeScale][page] : null); gl!.bufferData(gl!.ARRAY_BUFFER, data, gl!.DYNAMIC_DRAW);
      gl!.drawArraysInstanced(gl!.TRIANGLES, 0, 6, count);
    }
  }
  return {
    render(world, camera, waterFrame = 0, geometryPasses = []) {
      if (canvas.width < 1 || canvas.height < 1) throw new Error("Canvas dimensions must be positive");
      // The camera is in backing-store pixels, so its zoom already includes the device pixel ratio.
      const desiredScale = camera.zoom;
      const scales = Object.keys(atlas.pages).map(Number).sort((a, b) => a - b);
      activeScale = String(scales.find((scale) => scale >= desiredScale) ?? scales[scales.length - 1]);
      const viewport = { width: canvas.width, height: canvas.height };
      scene = buildScene(world, camera, viewport, atlas, activeScale, waterFrame);
      for (const geometryPass of geometryPasses) {
        const geometry = geometryPass(camera, viewport, atlas, activeScale);
        appendGeometry(scene, world.undergroundLevel === undefined ? geometry : clipGeometryAboveLevel(geometry, world.undergroundLevel, UNDERGROUND_CONTENT_DEPTH_BIAS), viewport);
      }
      if (scene.triangles.length / 2 >= 0xffffff) throw new Error("Too many visible objects for picking");
      surfaceScene = { triangles: scene.triangles.filter(({ kind }) => kind === "terrain" || kind === "carved-floor" || kind === "path") };
      prepare();
      if (pickWidth !== canvas.width || pickHeight !== canvas.height) {
        pickWidth = canvas.width; pickHeight = canvas.height;
        gl!.bindTexture(gl!.TEXTURE_2D, pickTexture); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.NEAREST); gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.NEAREST);
        gl!.texImage2D(gl!.TEXTURE_2D, 0, gl!.RGBA8, pickWidth, pickHeight, 0, gl!.RGBA, gl!.UNSIGNED_BYTE, null);
        gl!.bindRenderbuffer(gl!.RENDERBUFFER, depthBuffer); gl!.renderbufferStorage(gl!.RENDERBUFFER, gl!.DEPTH_COMPONENT24, pickWidth, pickHeight);
        gl!.bindFramebuffer(gl!.FRAMEBUFFER, framebuffer); gl!.framebufferTexture2D(gl!.FRAMEBUFFER, gl!.COLOR_ATTACHMENT0, gl!.TEXTURE_2D, pickTexture, 0);
        gl!.framebufferRenderbuffer(gl!.FRAMEBUFFER, gl!.DEPTH_ATTACHMENT, gl!.RENDERBUFFER, depthBuffer);
        if (gl!.checkFramebufferStatus(gl!.FRAMEBUFFER) !== gl!.FRAMEBUFFER_COMPLETE) throw new Error("Picking framebuffer is incomplete");
      }
      draw(false); draw(true); gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
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
