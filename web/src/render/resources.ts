import { GRADE_GLSL, POOL_GLSL } from "./daylight";

export const FLOATS_PER_QUAD = 28;

export function createRenderResources(gl: WebGL2RenderingContext, images: Record<string, HTMLImageElement[]>) {
  function shader(type: number, source: string): WebGLShader {
    const result = gl.createShader(type);
    if (!result) throw new Error("Could not create shader");
    gl.shaderSource(result, source); gl.compileShader(result);
    if (!gl.getShaderParameter(result, gl.COMPILE_STATUS)) throw new Error(`Shader compilation failed: ${gl.getShaderInfoLog(result)}`);
    return result;
  }
  function link(vertexSource: string, fragmentSource: string): WebGLProgram {
    const vertex = shader(gl.VERTEX_SHADER, vertexSource); const fragment = shader(gl.FRAGMENT_SHADER, fragmentSource);
    const result = gl.createProgram();
    if (!result) throw new Error("Could not create program");
    gl.attachShader(result, vertex); gl.attachShader(result, fragment); gl.linkProgram(result);
    if (!gl.getProgramParameter(result, gl.LINK_STATUS)) throw new Error(`Program link failed: ${gl.getProgramInfoLog(result)}`);
    gl.deleteShader(vertex); gl.deleteShader(fragment);
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
    uniform sampler2D poolLight; uniform bool pooled; uniform vec2 resolution;
    ${GRADE_GLSL}
    ${POOL_GLSL}
    in vec2 uv; flat in vec3 id; flat in vec4 fill; flat in float lit; out vec4 color;
    void main() {
      vec4 texel = textured ? texture(atlas,uv) * fill : fill;
      if (shape != 0) {
        float r = length(uv * 2.0 - 1.0);
        texel.a *= shape == 1 ? pow(max(0.0, 1.0 - r * r), 2.0) : shape == 2 ? 1.0 - smoothstep(0.75, 1.0, r) : 1.0 - smoothstep(0.35, 1.0, r);
      }
      if (texel.a < (shape != 0 ? 0.003 : 0.05) || (picking && all(equal(id, vec3(0.0))))) discard;
      vec3 printed = graded ? grade(texel.rgb, lit) : texel.rgb;
      if (pooled) printed = lightInPool(printed, texel.rgb, texture(poolLight, gl_FragCoord.xy / resolution).rgb);
      color = picking ? vec4(id,1.0) : vec4(printed, texel.a);
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
  const poolFramebuffer = gl.createFramebuffer(); const poolTexture = gl.createTexture();
  if (!vao || !emptyVao || !buffer || !framebuffer || !pickTexture || !depthBuffer || !glowFramebuffer || !glowTexture || !poolFramebuffer || !poolTexture) throw new Error("Could not create WebGL resources");
  gl.bindVertexArray(vao); gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const stride = FLOATS_PER_QUAD * 4;
  for (let i = 0; i < 4; i++) { gl.enableVertexAttribArray(i); gl.vertexAttribPointer(i, 4, gl.FLOAT, false, stride, i * 16); gl.vertexAttribDivisor(i, 1); }
  gl.enableVertexAttribArray(4); gl.vertexAttribPointer(4, 4, gl.FLOAT, false, stride, 64); gl.vertexAttribDivisor(4, 1);
  gl.enableVertexAttribArray(5); gl.vertexAttribPointer(5, 3, gl.FLOAT, false, stride, 80); gl.vertexAttribDivisor(5, 1);
  gl.enableVertexAttribArray(6); gl.vertexAttribPointer(6, 4, gl.FLOAT, false, stride, 92); gl.vertexAttribDivisor(6, 1);
  gl.enableVertexAttribArray(7); gl.vertexAttribPointer(7, 1, gl.FLOAT, false, stride, 108); gl.vertexAttribDivisor(7, 1);
  const textures = Object.fromEntries(Object.entries(images).map(([scale, pageImages]) => [scale, pageImages.map((image) => {
    const texture = gl.createTexture(); if (!texture) throw new Error("Could not create atlas texture");
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image); return texture;
  })]));
  return { program, skyProgram, glowProgram, vao, emptyVao, buffer, framebuffer, pickTexture, depthBuffer, glowFramebuffer, glowTexture, poolFramebuffer, poolTexture, textures };
}
