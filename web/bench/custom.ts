type Renderer = { adapter: string; render(data: Float32Array, count: number): void };

export async function createCustom(
  canvas: HTMLCanvasElement,
  backend: "webgpu" | "webgl2",
  atlas: HTMLCanvasElement,
): Promise<Renderer> {
  if (backend === "webgl2") return createWebGL(canvas, atlas);
  return createWebGPU(canvas, atlas);
}

function createWebGL(canvas: HTMLCanvasElement, atlas: HTMLCanvasElement): Renderer {
  const gl = canvas.getContext("webgl2", { alpha: false });
  if (!gl) throw new Error("WebGL2 is unavailable");

  function shader(type: number, source: string): WebGLShader {
    const result = gl!.createShader(type);
    if (!result) throw new Error("Could not create WebGL shader");
    gl!.shaderSource(result, source);
    gl!.compileShader(result);
    if (!gl!.getShaderParameter(result, gl!.COMPILE_STATUS)) {
      throw new Error(`WebGL shader compilation failed: ${gl!.getShaderInfoLog(result)}`);
    }
    return result;
  }

  const vertex = shader(gl.VERTEX_SHADER, `#version 300 es
    precision highp float;
    layout(location = 0) in vec4 rect;
    layout(location = 1) in vec4 sprite;
    uniform vec2 resolution;
    uniform vec2 atlasSize;
    out vec2 uv;
    void main() {
      vec2 corners[6] = vec2[6](vec2(0,0), vec2(1,0), vec2(0,1), vec2(0,1), vec2(1,0), vec2(1,1));
      vec2 corner = corners[gl_VertexID];
      vec2 position = (rect.xy + corner * rect.zw) / resolution;
      gl_Position = vec4(position.x * 2.0 - 1.0, 1.0 - position.y * 2.0, sprite.z * 2.0 - 1.0, 1.0);
      uv = (sprite.xy + corner * 16.0) / atlasSize;
    }`);
  const fragment = shader(gl.FRAGMENT_SHADER, `#version 300 es
    precision highp float;
    uniform sampler2D atlas;
    in vec2 uv;
    out vec4 color;
    void main() {
      color = texture(atlas, uv);
      if (color.a < 0.5) discard;
    }`);
  const program = gl.createProgram();
  if (!program) throw new Error("Could not create WebGL program");
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(`WebGL program link failed: ${gl.getProgramInfoLog(program)}`);
  }
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  const buffer = gl.createBuffer();
  const texture = gl.createTexture();
  const vao = gl.createVertexArray();
  if (!buffer || !texture || !vao) throw new Error("Could not create WebGL resources");
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  for (let index = 0; index < 2; index++) {
    gl.enableVertexAttribArray(index);
    gl.vertexAttribPointer(index, 4, gl.FLOAT, false, 32, index * 16);
    gl.vertexAttribDivisor(index, 1);
  }
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, atlas);
  gl.useProgram(program);
  gl.uniform1i(gl.getUniformLocation(program, "atlas"), 0);
  const resolution = gl.getUniformLocation(program, "resolution");
  gl.uniform2f(gl.getUniformLocation(program, "atlasSize"), atlas.width, atlas.height);
  gl.enable(gl.DEPTH_TEST);
  gl.depthFunc(gl.LEQUAL);
  gl.enable(gl.BLEND);
  gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
  const vendor = String(gl.getParameter(debugInfo ? debugInfo.UNMASKED_VENDOR_WEBGL : gl.VENDOR));
  const renderer = String(gl.getParameter(debugInfo ? debugInfo.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
  return {
    adapter: `${vendor} ${renderer}`,
    render(data, count) {
      if (count < 0 || !Number.isInteger(count) || count * 8 > data.length) {
        throw new Error("Invalid sprite count");
      }
      gl!.viewport(0, 0, canvas.width, canvas.height);
      gl!.clearColor(0, 0, 0, 1);
      gl!.clearDepth(1);
      gl!.clear(gl!.COLOR_BUFFER_BIT | gl!.DEPTH_BUFFER_BIT);
      gl!.useProgram(program);
      gl!.uniform2f(resolution, canvas.width, canvas.height);
      gl!.activeTexture(gl!.TEXTURE0);
      gl!.bindTexture(gl!.TEXTURE_2D, texture);
      gl!.bindVertexArray(vao);
      gl!.bindBuffer(gl!.ARRAY_BUFFER, buffer);
      gl!.bufferData(gl!.ARRAY_BUFFER, data.subarray(0, count * 8), gl!.DYNAMIC_DRAW);
      gl!.drawArraysInstanced(gl!.TRIANGLES, 0, 6, count);
    },
  };
}

async function createWebGPU(canvas: HTMLCanvasElement, atlas: HTMLCanvasElement): Promise<Renderer> {
  const gpu = navigator.gpu;
  if (!gpu) throw new Error("WebGPU is unavailable");
  const adapter = await gpu.requestAdapter();
  if (!adapter) throw new Error("No WebGPU adapter available");
  const device = await adapter.requestDevice();
  const adapterName = [adapter.info.vendor, adapter.info.architecture, adapter.info.device, adapter.info.description].filter(Boolean).join(" ") || "WebGPU adapter (identity unavailable)";
  const context = canvas.getContext("webgpu" as "2d") as unknown as GPUCanvasContext | null;
  if (!context) throw new Error("Could not create WebGPU canvas context");
  const format = gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "opaque" });
  const bufferUsage = { copyDst: 0x08, vertex: 0x20, uniform: 0x40 };
  const textureUsage = { copyDst: 0x02, textureBinding: 0x04, renderAttachment: 0x10 };
  const texture = device.createTexture({
    size: [atlas.width, atlas.height], format: "rgba8unorm",
    usage: textureUsage.copyDst | textureUsage.textureBinding | textureUsage.renderAttachment,
  });
  device.queue.copyExternalImageToTexture({ source: atlas }, { texture }, [atlas.width, atlas.height]);
  const sampler = device.createSampler({ magFilter: "nearest", minFilter: "nearest" });
  const resolution = device.createBuffer({ size: 8, usage: bufferUsage.uniform | bufferUsage.copyDst });
  const shader = device.createShaderModule({ code: `
    struct VertexOutput { @builtin(position) position: vec4f, @location(0) uv: vec2f };
    @group(0) @binding(0) var image: texture_2d<f32>;
    @group(0) @binding(1) var imageSampler: sampler;
    @group(0) @binding(2) var<uniform> resolution: vec2f;
    @vertex fn vertex(@builtin(vertex_index) vertexIndex: u32, @location(0) rect: vec4f, @location(1) sprite: vec4f) -> VertexOutput {
      let corners = array<vec2f, 6>(vec2f(0,0), vec2f(1,0), vec2f(0,1), vec2f(0,1), vec2f(1,0), vec2f(1,1));
      let corner = corners[vertexIndex];
      let position = (rect.xy + corner * rect.zw) / resolution;
      var output: VertexOutput;
      output.position = vec4f(position.x * 2.0 - 1.0, 1.0 - position.y * 2.0, sprite.z, 1.0);
      output.uv = (sprite.xy + corner * 16.0) / vec2f(textureDimensions(image));
      return output;
    }
    @fragment fn fragment(input: VertexOutput) -> @location(0) vec4f {
      let color = textureSample(image, imageSampler, input.uv);
      if (color.a < 0.5) { discard; }
      return color;
    }` });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module: shader, entryPoint: "vertex", buffers: [{
      arrayStride: 32, stepMode: "instance", attributes: [
        { shaderLocation: 0, offset: 0, format: "float32x4" },
        { shaderLocation: 1, offset: 16, format: "float32x4" },
      ],
    }] },
    fragment: { module: shader, entryPoint: "fragment", targets: [{ format, blend: {
      color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" },
      alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
    } }] },
    primitive: { topology: "triangle-list" },
    depthStencil: { format: "depth24plus", depthWriteEnabled: true, depthCompare: "less-equal" },
  });
  const bindGroup = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [
    { binding: 0, resource: texture.createView() },
    { binding: 1, resource: sampler },
    { binding: 2, resource: { buffer: resolution } },
  ] });
  let capacity = 32;
  let instances = device.createBuffer({ size: capacity, usage: bufferUsage.vertex | bufferUsage.copyDst });
  let depth: GPUTexture | undefined;
  let depthWidth = 0;
  let depthHeight = 0;
  return {
    adapter: adapterName,
    render(data, count) {
      if (count < 0 || !Number.isInteger(count) || count * 8 > data.length) {
        throw new Error("Invalid sprite count");
      }
      const bytes = count * 32;
      if (bytes > capacity) {
        instances.destroy();
        capacity = bytes;
        instances = device.createBuffer({ size: capacity, usage: bufferUsage.vertex | bufferUsage.copyDst });
      }
      if (canvas.width !== depthWidth || canvas.height !== depthHeight) {
        if (depth) depth.destroy();
        depthWidth = canvas.width;
        depthHeight = canvas.height;
        depth = device.createTexture({ size: [depthWidth, depthHeight], format: "depth24plus", usage: textureUsage.renderAttachment });
      }
      if (!depth) throw new Error("Could not create WebGPU depth texture");
      if (bytes) device.queue.writeBuffer(instances, 0, data.buffer, data.byteOffset, bytes);
      device.queue.writeBuffer(resolution, 0, new Float32Array([canvas.width, canvas.height]));
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [{ view: context.getCurrentTexture().createView(), clearValue: { r: 0, g: 0, b: 0, a: 1 }, loadOp: "clear", storeOp: "store" }],
        depthStencilAttachment: { view: depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" },
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.setVertexBuffer(0, instances);
      pass.draw(6, count);
      pass.end();
      device.queue.submit([encoder.finish()]);
    },
  };
}
