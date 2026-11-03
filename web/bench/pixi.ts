import { Application, Particle, ParticleContainer, Rectangle, Texture } from "pixi.js";

const ATLAS_CELL_SIZE = 16;
const INSTANCE_STRIDE = 8;

type DepthParticle = Particle & {
  depth: number;
  sourceIndex: number;
};

export async function createPixi(
  canvas: HTMLCanvasElement,
  backend: "webgpu" | "webgl2",
  atlas: HTMLCanvasElement,
): Promise<{ adapter: string; render(data: Float32Array, count: number): void }> {
  if (atlas.width < ATLAS_CELL_SIZE || atlas.height < ATLAS_CELL_SIZE) {
    throw new RangeError("Pixi atlas must contain at least one 16×16 cell");
  }
  if (atlas.width % ATLAS_CELL_SIZE !== 0 || atlas.height % ATLAS_CELL_SIZE !== 0) {
    throw new RangeError("Pixi atlas dimensions must be multiples of 16 pixels");
  }

  const atlasColumns = atlas.width / ATLAS_CELL_SIZE;
  const atlasRows = atlas.height / ATLAS_CELL_SIZE;
  const atlasTexture = Texture.from(atlas);
  atlasTexture.source.scaleMode = "nearest";
  const cellTextures: Texture[] = [];
  for (let row = 0; row < atlasRows; row += 1) {
    for (let column = 0; column < atlasColumns; column += 1) {
      cellTextures.push(
        new Texture({
          source: atlasTexture.source,
          frame: new Rectangle(
            column * ATLAS_CELL_SIZE,
            row * ATLAS_CELL_SIZE,
            ATLAS_CELL_SIZE,
            ATLAS_CELL_SIZE,
          ),
        }),
      );
    }
  }

  const app = new Application();
  await app.init({
    canvas,
    width: canvas.width,
    height: canvas.height,
    autoStart: false,
    autoDensity: false,
    resolution: 1,
    antialias: false,
    backgroundColor: 0x000000,
    preference: backend === "webgpu" ? ["webgpu"] : ["webgl"],
    preferWebGLVersion: 2,
  });
  app.ticker.stop();
  const adapter = getAdapterLabel(app.renderer, backend);

  const particles = new ParticleContainer<DepthParticle>({
    texture: atlasTexture,
    boundsArea: new Rectangle(0, 0, canvas.width, canvas.height),
    dynamicProperties: {
      position: true,
      vertex: true,
      rotation: true,
      uvs: true,
      color: true,
    },
  });
  app.stage.addChild(particles);

  const particlePool: DepthParticle[] = [];
  const painterOrder: DepthParticle[] = [];

  return {
    adapter,
    render(data: Float32Array, count: number): void {
      if (!Number.isInteger(count) || count < 0) {
        throw new RangeError("Pixi instance count must be a non-negative integer");
      }
      if (count * INSTANCE_STRIDE > data.length) {
        throw new RangeError("Pixi instance data does not contain count × 8 floats");
      }

      while (particlePool.length < count) {
        const sourceIndex = particlePool.length;
        particlePool.push(
          Object.assign(
            new Particle({
              texture: cellTextures[0]!,
              anchorX: 0,
              anchorY: 0,
            }),
            { depth: 0, sourceIndex },
          ),
        );
      }

      const children = particles.particleChildren;
      children.length = count;
      painterOrder.length = count;
      for (let instanceIndex = 0; instanceIndex < count; instanceIndex += 1) {
        const dataOffset = instanceIndex * INSTANCE_STRIDE;
        const x = data[dataOffset]!;
        const y = data[dataOffset + 1]!;
        const width = data[dataOffset + 2]!;
        const height = data[dataOffset + 3]!;
        const atlasU = data[dataOffset + 4]!;
        const atlasV = data[dataOffset + 5]!;
        const depth = data[dataOffset + 6]!;
        const column = atlasU / ATLAS_CELL_SIZE;
        const row = atlasV / ATLAS_CELL_SIZE;

        if (
          !Number.isInteger(column) ||
          !Number.isInteger(row) ||
          column < 0 ||
          column >= atlasColumns ||
          row < 0 ||
          row >= atlasRows
        ) {
          throw new RangeError(`Pixi atlas cell origin (${atlasU}, ${atlasV}) is out of bounds`);
        }
        if (!Number.isFinite(depth)) {
          throw new RangeError(`Pixi instance ${instanceIndex} has a non-finite depth`);
        }

        const particle = particlePool[instanceIndex]!;
        particle.x = x;
        particle.y = y;
        particle.scaleX = width / ATLAS_CELL_SIZE;
        particle.scaleY = height / ATLAS_CELL_SIZE;
        particle.texture = cellTextures[row * atlasColumns + column]!;
        particle.depth = depth;
        children[instanceIndex] = particle;
        painterOrder[instanceIndex] = particle;
      }

      painterOrder.sort((first, second) => {
        if (first.depth !== second.depth) return first.depth > second.depth ? -1 : 1;
        return first.sourceIndex - second.sourceIndex;
      });
      for (let index = 0; index < count; index += 1) {
        children[index] = painterOrder[index]!;
      }

      particles.update();
      app.render();
    },
  };
}

function getAdapterLabel(
  renderer: Application["renderer"],
  backend: "webgpu" | "webgl2",
): string {
  if (backend === "webgpu") {
    if (!("gpu" in renderer)) throw new Error("Pixi did not initialize its WebGPU renderer");
    const info = renderer.gpu.adapter.info;
    return [info.vendor, info.architecture, info.device, info.description].filter(Boolean).join(" ") || "WebGPU adapter";
  }

  if (!("gl" in renderer)) throw new Error("Pixi did not initialize its WebGL renderer");
  const gl = renderer.gl;
  const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
  const vendor = gl.getParameter(debugInfo?.UNMASKED_VENDOR_WEBGL ?? gl.VENDOR);
  const device = gl.getParameter(debugInfo?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER);
  return [vendor, device].map(String).filter(Boolean).join(" ") || "WebGL2";
}
