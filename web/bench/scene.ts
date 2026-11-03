export const WIDTH = 1920;
export const HEIGHT = 1080;
const STRIDE = 8;
const TERRAIN_SIZE = 256;
const PEOPLE = 10_000;
const TRACK_SEGMENTS = 2_048;
const TERRAIN_COUNT = TERRAIN_SIZE * TERRAIN_SIZE;
const MARKER_COUNT = 12;
const TUNNEL_COUNT = 3;

export const markers = {
  terrain: { x: 40, y: 40, color: [68, 150, 76, 255] },
  person: { x: 80, y: 40, color: [238, 95, 75, 255] },
  track: { x: 120, y: 40, color: [249, 202, 70, 255] },
  tunnel: { x: 160, y: 40, color: [64, 73, 92, 255] },
  trackOverTrack: { x: 200, y: 40, color: [249, 202, 70, 255] },
  transparentEdge: { x: 240, y: 40, color: [249, 202, 70, 255] },
} as const;

export function createAtlas(): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = 96;
  canvas.height = 16;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas 2D is unavailable");
  context.fillStyle = "rgb(68, 150, 76)";
  context.beginPath();
  context.moveTo(8, 0);
  context.lineTo(16, 8);
  context.lineTo(8, 16);
  context.lineTo(0, 8);
  context.closePath();
  context.fill();
  context.fillStyle = "rgb(238, 95, 75)";
  for (const [cell, legOffset] of [[1, 0], [4, -2], [5, 2]] as const) {
    const left = cell * 16;
    context.fillRect(left + 5, 1, 6, 4);
    context.fillRect(left + 4, 5, 8, 7);
    context.fillRect(left + 5 + legOffset, 12, 3, 4);
    context.fillRect(left + 9 - legOffset, 12, 3, 4);
  }
  context.fillStyle = "rgb(249, 202, 70)";
  context.fillRect(32, 5, 16, 6);
  context.fillStyle = "rgb(64, 73, 92)";
  context.fillRect(48, 3, 16, 13);
  return canvas;
}

function writeSprite(data: Float32Array, index: number, x: number, y: number,
  width: number, height: number, cell: number, depth: number): void {
  const offset = index * STRIDE;
  data[offset] = x;
  data[offset + 1] = y;
  data[offset + 2] = width;
  data[offset + 3] = height;
  data[offset + 4] = cell * 16;
  data[offset + 5] = 0;
  data[offset + 6] = depth;
  data[offset + 7] = 0;
}

function projectX(x: number, y: number): number {
  return WIDTH / 2 + (x - y) * 3.5;
}

function projectY(x: number, y: number): number {
  return 65 + (x + y) * 1.75;
}

export class Scene {
  readonly count = TERRAIN_COUNT + PEOPLE + TRACK_SEGMENTS + TUNNEL_COUNT + MARKER_COUNT;
  readonly data = new Float32Array(this.count * STRIDE);

  constructor() {
    for (let y = 0; y < TERRAIN_SIZE; y++) {
      for (let x = 0; x < TERRAIN_SIZE; x++) {
        const index = y * TERRAIN_SIZE + x;
        writeSprite(this.data, index, projectX(x, y), projectY(x, y), 7, 4, 0,
          0.95 - (x + y) / 1200);
      }
    }
    // A long, overlapping track winds through the park and crosses itself.
    for (let index = 0; index < TRACK_SEGMENTS; index++) {
      const angle = index / TRACK_SEGMENTS * Math.PI * 12;
      const x = 128 + Math.sin(angle) * 92;
      const y = 128 + Math.sin(angle * 0.5) * 92;
      const height = 18 + 16 * Math.sin(angle * 2);
      writeSprite(this.data, TERRAIN_COUNT + PEOPLE + index,
        projectX(x, y), projectY(x, y) - height, 9, 5,
        2,
        0.94 - (x + y) / 1200 - height / 3000);
    }
    const tunnelStart = TERRAIN_COUNT + PEOPLE + TRACK_SEGMENTS;
    writeSprite(this.data, tunnelStart, 900, 495, 180, 12, 2, 0.55);
    writeSprite(this.data, tunnelStart + 1, 970, 460, 110, 55, 0, 0.50);
    writeSprite(this.data, tunnelStart + 2, 960, 485, 25, 30, 3, 0.49);
    for (const [index, marker] of Object.values(markers).entries()) {
      const cell = index === 4 ? 2 : index === 5 ? 0 : index;
      writeSprite(this.data, tunnelStart + TUNNEL_COUNT + index,
        marker.x - (index === 5 ? 2 : 8), marker.y - (index === 5 ? 2 : 8),
        16, 16, cell, 0);
      // Draw the farther sprite later so each marker also checks depth ordering.
      writeSprite(this.data, tunnelStart + TUNNEL_COUNT + 6 + index,
        marker.x - 8, marker.y - 8, 16, 16, index === 2 ? 1 : index === 4 ? 3 : 2, 0.1);
    }
    this.update(0);
  }

  update(elapsedMs: number): void {
    for (let index = 0; index < PEOPLE; index++) {
      const phase = elapsedMs * 0.0003 + index * 0.071;
      const x = 8 + ((index * 73) % 240) + Math.sin(phase) * 2;
      const y = 8 + ((index * 131) % 240) + Math.cos(phase) * 2;
      const bob = Math.sin(phase * 8) * 1.5;
      writeSprite(this.data, TERRAIN_COUNT + index, projectX(x, y),
        projectY(x, y) - 7 + bob, 5, 8,
        [1, 4, 1, 5][Math.floor(phase * 8) % 4], 0.94 - (x + y) / 1200);
    }
  }
}
