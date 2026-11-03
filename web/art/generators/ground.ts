// Ground tiles, cliffs and sky pieces. Tiles are slightly oversized so that no background shows
// through the seams when the camera zooms to fractional sizes.
import { TILE_SCREEN_HEIGHT as TILE_H, TILE_SCREEN_WIDTH as TILE_W } from "./iso.ts";
import { hatchQuad, INK, inkLine, type Point, seededRandom, shadePlum, smoothPath, subdivide, wobble } from "./ink.ts";
import { COLORS } from "./palette.ts";

const HALF_W = TILE_W / 2;
const HALF_H = TILE_H / 2;
const SEAM_OVERLAP = 0.8;
export const TILE_CANVAS = { width: 72, height: 40, anchorX: 36, anchorY: 20 };

/** The tile's corners in screen order: top (-x,-y), right (+x,-y), bottom (+x,+y), left (-x,+y). */
function diamond(extra = SEAM_OVERLAP): [Point, Point, Point, Point] {
  return [[0, -HALF_H - extra / 2], [HALF_W + extra, 0], [0, HALF_H + extra / 2], [-HALF_W - extra, 0]];
}

function pathOf(points: readonly Point[]): string {
  return `M${points.map(([x, y]) => `${x} ${y}`).join("L")}Z`;
}

export function drawGrass(variant: number, color: string): string {
  const random = seededRandom(`grass${variant}${color}`);
  const parts = [`<path d="${pathOf(diamond())}" fill="${color}"/>`];
  // A faint grid on the two back edges, as in the mockup.
  const [top, right, , left] = diamond(0);
  parts.push(inkLine([left, top, right], random, { width: 0.7, opacity: 0.14 }));
  for (let tuft = 0; tuft < 3 + variant; tuft++) {
    const spreadX = (random() - 0.5) * 34;
    const spreadY = (random() - 0.5) * (16 - Math.abs(spreadX) * 0.45);
    parts.push(
      `<path d="M${spreadX - 1.5} ${spreadY}q0.4 -2 -0.4 -3M${spreadX} ${spreadY}q0.2 -2.6 0.4 -3.6M${spreadX + 1.5} ${spreadY}q0 -2 1 -2.8" fill="none" stroke="${shadePlum(color, 0.42)}" stroke-width="0.7" stroke-linecap="round"/>`,
    );
  }
  if (variant === 2) {
    const [flowerX, flowerY] = [(random() - 0.5) * 20, (random() - 0.5) * 7];
    parts.push(`<circle cx="${flowerX}" cy="${flowerY}" r="1.3" fill="${COLORS.paper}" stroke="${INK}" stroke-width="0.35"/><circle cx="${flowerX}" cy="${flowerY}" r="0.5" fill="${COLORS.yellow}"/>`);
  }
  return parts.join("\n");
}

/**
 * A path tile; `connections` lists which of +x, +y, -x, -y have a neighbouring path.
 * Edges without a neighbour get a kerb, so paths draw their own outline as they turn.
 */
export function drawPath(connections: readonly [boolean, boolean, boolean, boolean]): string {
  const [connectedPlusX, connectedPlusY, connectedMinusX, connectedMinusY] = connections;
  const random = seededRandom(`path${connections.join()}`);
  const [top, right, bottom, left] = diamond(0);
  const parts = [`<path d="${pathOf(diamond())}" fill="${COLORS.path}"/>`];
  // +x edge runs from the right to the bottom corner, +y from bottom to left, -x from left to top, -y from top to right.
  const edges: [boolean, Point, Point][] = [
    [connectedPlusX, right, bottom],
    [connectedPlusY, bottom, left],
    [connectedMinusX, left, top],
    [connectedMinusY, top, right],
  ];
  for (const [connected, start, end] of edges) {
    if (connected) continue;
    const inward = 0.86;
    const innerStart: Point = [start[0] * inward, start[1] * inward];
    const innerEnd: Point = [end[0] * inward, end[1] * inward];
    parts.push(`<path d="${pathOf([start, end, innerEnd, innerStart])}" fill="${COLORS.peach}"/>`);
    parts.push(inkLine([start, end], random, { width: 1.1, taper: false }));
    parts.push(inkLine([innerStart, innerEnd], random, { width: 0.6, opacity: 0.45 }));
  }
  for (let pebble = 0; pebble < 5; pebble++) {
    const pebbleX = (random() - 0.5) * 30;
    const pebbleY = (random() - 0.5) * (13 - Math.abs(pebbleX) * 0.4);
    parts.push(`<ellipse cx="${pebbleX}" cy="${pebbleY}" rx="${1 + random() * 1.1}" ry="${0.55 + random() * 0.4}" fill="${shadePlum(COLORS.path, 0.2)}"/>`);
  }
  return parts.join("\n");
}

export const CLIFF_CANVAS = { width: 40, height: 76, anchorX: 2, anchorY: 2 };

/** A hatched earth wall hanging below one tile edge. `side` picks the right-hand (+x) or left-hand (+y) face. */
export function drawCliff(side: "right" | "left", depth: number, variant: number): string {
  const random = seededRandom(`cliff${side}${variant}`);
  const color = side === "right" ? COLORS.earth : shadePlum(COLORS.earth, 0.2);
  // Drawn from the corner at the anchor. Right face: going down-left. Left face: going down-right.
  const direction = side === "right" ? -1 : 1;
  const [topStartX, topStartY, topEndX, topEndY] = [0, 0, direction * HALF_W, HALF_H];
  const lumpDepthStart = depth + (variant % 3) * 3;
  const lumpDepthEnd = depth + ((variant + 1) % 3) * 3;
  const startBottom: Point = [topStartX, topStartY + lumpDepthStart];
  const endBottom: Point = [topEndX, topEndY + lumpDepthEnd];
  const corners: [Point, Point, Point, Point] = [[topStartX, topStartY], [topEndX, topEndY], endBottom, startBottom];
  const parts = [`<path d="${smoothPath(wobble(subdivide(corners, true, 8), 0.3, random), true, 0.4)}" fill="${color}" stroke="${color}" stroke-width="1"/>`];
  parts.push(hatchQuad(corners, 8, random, { opacity: 0.42, width: 0.8 }));
  parts.push(inkLine([[topStartX, topStartY], [topEndX, topEndY]], random, { width: 1.25, taper: false }));
  parts.push(inkLine([startBottom, endBottom], random, { width: 1.25, taper: false }));
  if (variant === 1) {
    const rootX = topEndX * 0.5;
    const rootTop = topEndY * 0.5 + depth * 0.8;
    parts.push(`<path d="M${rootX} ${rootTop}q2 8 -1 13q-2 4 1 6" fill="none" stroke="${INK}" stroke-width="1" stroke-linecap="round"/>`);
  }
  return parts.join("\n");
}

export function drawSun(): string {
  const random = seededRandom("sun");
  const parts: string[] = [];
  for (let ray = 0; ray < 12; ray++) {
    const angle = (ray / 12) * Math.PI * 2 + (random() - 0.5) * 0.08;
    const [inner, outer] = [68 + random() * 3, ray % 2 ? 88 : 100];
    parts.push(inkLine([[Math.cos(angle) * inner, Math.sin(angle) * inner], [Math.cos(angle) * outer, Math.sin(angle) * outer]], random, { width: 5.2, color: COLORS.leafYellow }));
  }
  const disc = Array.from({ length: 22 }, (_, index): Point => {
    const angle = (index / 22) * Math.PI * 2;
    return [Math.cos(angle) * 58, Math.sin(angle) * 58];
  });
  parts.push(`<path d="${smoothPath(wobble(disc, 1.2, random, 40), true)}" fill="#ffd95e" stroke="${INK}" stroke-width="1.6"/>`);
  parts.push(`<ellipse cx="-20" cy="-8" rx="4" ry="5.5" fill="${INK}"/><ellipse cx="20" cy="-8" rx="4" ry="5.5" fill="${INK}"/>`);
  parts.push(`<ellipse cx="-33" cy="11" rx="9" ry="5" fill="#ff8a6c" fill-opacity="0.55"/><ellipse cx="33" cy="11" rx="9" ry="5" fill="#ff8a6c" fill-opacity="0.55"/>`);
  parts.push(`<path d="M-17 13Q0 32 17 13" fill="none" stroke="${INK}" stroke-width="2.6" stroke-linecap="round"/>`);
  return parts.join("\n");
}

export function drawCloud(puffs: readonly (readonly [number, number, number])[], seed: string): string {
  const random = seededRandom(seed);
  const shape = (extra: number, fill: string): string =>
    puffs
      .map(([x, y, radius]) => {
        const points = Array.from({ length: 12 }, (_, index): Point => {
          const angle = (index / 12) * Math.PI * 2;
          return [x + Math.cos(angle) * (radius + extra), y + Math.sin(angle) * (radius + extra)];
        });
        return `<path d="${smoothPath(wobble(points, 0.5, random, 20), true)}" fill="${fill}"/>`;
      })
      .join("");
  return shape(1.3, INK) + shape(0, "#fff8ee");
}
