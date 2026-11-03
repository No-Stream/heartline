// Ground tiles, cliffs and sky pieces. Tiles are slightly oversized so that no background shows
// through the seams when the camera zooms to fractional sizes.
//
// Sprite names the game renderer depends on, in every theme (keep them stable):
//   ground/grass_light_<0-2>, ground/grass_dark_<0-2>   grass tiles, 72x40 canvas, anchor at the tile centre
//   ground/path_<bits>                                  path tiles; bits are +x +y -x -y neighbours, e.g. path_1010
//   ground/water                                        water tile on the same canvas as grass: view "any",
//                                                       animation "static" frame 0, and "loop" frames 0-3 with moving ripples
//   ground/cliff_step_right_<0-2>, ground/cliff_step_left_<0-2>
//                                                       earth face one height step tall (8 px, a quarter tile edge)
//                                                       under the right-hand (+x) or left-hand (+y) tile edge; stack them
//                                                       for bigger steps. Anchor is the top corner of the edge.
//   ground/cliff_right_<0-2>, ground/cliff_left_<0-2>   deep map-edge faces (44 px) under the island's outer edges,
//                                                       with the same anchor convention as the step faces
import { TILE_SCREEN_HEIGHT as TILE_H, TILE_SCREEN_WIDTH as TILE_W } from "./iso.ts";
import { fillPath, formatNumber, hatchQuad, inkLine, type Point, seededRandom, shadePlum, smoothPath, subdivide, wobble } from "./ink.ts";
import { COLORS } from "./palette.ts";
import type { Theme } from "./theme.ts";

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

/** `color` is a design colour. Tiles are always flat colour, because a wash per tile would show the grid. */
export function drawGrass(theme: Theme, variant: number, color: string): string {
  const random = seededRandom(`grass${variant}${color}`);
  const parts = [`<path d="${pathOf(diamond())}" fill="${theme.color(color)}"/>`];
  // A faint grid on the two back edges, as in the mockup.
  const [top, right, , left] = diamond(0);
  if (theme.tileGridOpacity > 0) {
    parts.push(inkLine([left, top, right], random, { width: 0.7 * theme.line.weight, opacity: theme.tileGridOpacity, color: theme.line.color }));
  }
  for (let tuft = 0; tuft < 3 + variant; tuft++) {
    const spreadX = (random() - 0.5) * 34;
    const spreadY = (random() - 0.5) * (16 - Math.abs(spreadX) * 0.45);
    parts.push(
      `<path d="M${spreadX - 1.5} ${spreadY}q0.4 -2 -0.4 -3M${spreadX} ${spreadY}q0.2 -2.6 0.4 -3.6M${spreadX + 1.5} ${spreadY}q0 -2 1 -2.8" fill="none" stroke="${theme.shade(theme.color(color), 0.42)}" stroke-width="0.7" stroke-linecap="round"/>`,
    );
  }
  if (variant === 2) {
    const [flowerX, flowerY] = [(random() - 0.5) * 20, (random() - 0.5) * 7];
    parts.push(`<circle cx="${flowerX}" cy="${flowerY}" r="1.3" fill="${theme.color(COLORS.paper)}" stroke="${theme.line.color}" stroke-width="${0.35 * theme.line.weight}"/><circle cx="${flowerX}" cy="${flowerY}" r="0.5" fill="${theme.color(COLORS.yellow)}"/>`);
  }
  return parts.join("\n");
}

/**
 * A path tile; `connections` lists which of +x, +y, -x, -y have a neighbouring path.
 * Edges without a neighbour get a kerb, so paths draw their own outline as they turn.
 */
export function drawPath(theme: Theme, connections: readonly [boolean, boolean, boolean, boolean]): string {
  const [connectedPlusX, connectedPlusY, connectedMinusX, connectedMinusY] = connections;
  const random = seededRandom(`path${connections.join()}`);
  const [top, right, bottom, left] = diamond(0);
  const parts = [`<path d="${pathOf(diamond())}" fill="${theme.color(COLORS.path)}"/>`];
  const line = { color: theme.line.color, wobble: theme.line.wobble };
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
    parts.push(`<path d="${pathOf([start, end, innerEnd, innerStart])}" fill="${theme.color(COLORS.peach)}"/>`);
    parts.push(inkLine([start, end], random, { ...line, width: 1.1 * theme.line.weight, taper: false }));
    parts.push(inkLine([innerStart, innerEnd], random, { ...line, width: 0.6 * theme.line.weight, opacity: 0.45 }));
  }
  for (let pebble = 0; pebble < 5; pebble++) {
    const pebbleX = (random() - 0.5) * 30;
    const pebbleY = (random() - 0.5) * (13 - Math.abs(pebbleX) * 0.4);
    parts.push(`<ellipse cx="${pebbleX}" cy="${pebbleY}" rx="${1 + random() * 1.1}" ry="${0.55 + random() * 0.4}" fill="${theme.color(shadePlum(COLORS.path, 0.2))}"/>`);
  }
  return parts.join("\n");
}

export const CLIFF_CANVAS = { width: 40, height: 76, anchorX: 2, anchorY: 2 };
/** One height step of terrain, in screen pixels. */
export const HEIGHT_STEP_PIXELS = 8;
export const CLIFF_STEP_CANVAS = { width: 44, height: 36, anchorX: 4, anchorY: 4 };

/**
 * A hatched earth wall hanging below one tile edge. `side` picks the right-hand (+x) or left-hand (+y) face.
 * `lumpSize` is how much the bottom edge varies between variants, so neighbouring faces do not line up exactly.
 */
export function drawCliff(theme: Theme, side: "right" | "left", depth: number, variant: number, lumpSize = 3): string {
  const random = seededRandom(`cliff${side}${variant}`);
  const earth = theme.color(COLORS.earth);
  const color = side === "right" ? earth : theme.shade(earth, 0.2);
  const line = { color: theme.line.color, wobble: theme.line.wobble };
  // Drawn from the corner at the anchor. Right face: going down-left. Left face: going down-right.
  const direction = side === "right" ? -1 : 1;
  const [topStartX, topStartY, topEndX, topEndY] = [0, 0, direction * HALF_W, HALF_H];
  const lumpDepthStart = depth + (variant % 3) * lumpSize;
  const lumpDepthEnd = depth + ((variant + 1) % 3) * lumpSize;
  const startBottom: Point = [topStartX, topStartY + lumpDepthStart];
  const endBottom: Point = [topEndX, topEndY + lumpDepthEnd];
  const corners: [Point, Point, Point, Point] = [[topStartX, topStartY], [topEndX, topEndY], endBottom, startBottom];
  const parts = [`<path d="${smoothPath(wobble(subdivide(corners, true, 8), 0.3, random), true, 0.4)}" fill="${color}" stroke="${color}" stroke-width="1"/>`];
  const hatchLines = Math.round(Math.min(8, depth / 5) * theme.hatching);
  if (hatchLines > 0) parts.push(hatchQuad(corners, hatchLines, random, { opacity: 0.42, width: 0.8 * theme.line.weight, color: theme.line.color }));
  parts.push(inkLine([[topStartX, topStartY], [topEndX, topEndY]], random, { ...line, width: 1.25 * theme.line.weight, taper: false }));
  parts.push(inkLine([startBottom, endBottom], random, { ...line, width: 1.25 * theme.line.weight, taper: false }));
  if (variant === 1 && depth > 20) {
    const rootX = topEndX * 0.5;
    const rootTop = topEndY * 0.5 + depth * 0.8;
    parts.push(`<path d="M${rootX} ${rootTop}q2 8 -1 13q-2 4 1 6" fill="none" stroke="${theme.line.color}" stroke-width="${theme.line.weight}" stroke-linecap="round"/>`);
  }
  return parts.join("\n");
}

/** A water tile with ripples that drift across it over `frameCount` frames. */
export function drawWater(theme: Theme, frame: number, frameCount: number): string {
  const random = seededRandom("water");
  const water = theme.color(COLORS.water);
  const parts = [`<path d="${pathOf(diamond())}" fill="${water}"/>`];
  const [top, right, bottom, left] = diamond(0);
  const ripple = theme.color("#ffffff");
  for (let index = 0; index < 5; index++) {
    // Each ripple sits at a fixed spot and slides a little along the tile's +x axis as the frames run.
    const along = random() - 0.5 + (frame / frameCount) * 0.25;
    const across = random() - 0.5;
    const centerX = (along - across) * HALF_W * 0.6;
    const centerY = (along + across) * HALF_H * 0.6;
    const length = 3 + random() * 4;
    parts.push(`<path d="M${formatNumber(centerX - length)} ${formatNumber(centerY)}q${formatNumber(length / 2)} -1.4 ${formatNumber(length)} 0t${formatNumber(length)} 0" fill="none" stroke="${ripple}" stroke-opacity="0.55" stroke-width="0.8" stroke-linecap="round"/>`);
  }
  // A slightly deeper tone along the two far edges keeps neighbouring water tiles from merging into one flat sheet.
  parts.push(`<path d="${pathOf([left, top, right, [right[0] * 0.9, right[1] + 1.2], [0, top[1] * 0.85], [left[0] * 0.9, left[1] + 1.2]])}" fill="${theme.shade(water, 0.18)}" fill-opacity="0.5"/>`);
  void bottom;
  return parts.join("\n");
}

export function drawSun(theme: Theme): string {
  const random = seededRandom("sun");
  const parts: string[] = [];
  for (let ray = 0; ray < 12; ray++) {
    const angle = (ray / 12) * Math.PI * 2 + (random() - 0.5) * 0.08;
    const [inner, outer] = [68 + random() * 3, ray % 2 ? 88 : 100];
    if (theme.props.sunFace) {
      parts.push(inkLine([[Math.cos(angle) * inner, Math.sin(angle) * inner], [Math.cos(angle) * outer, Math.sin(angle) * outer]], random, { width: 5.2, color: theme.color(COLORS.leafYellow) }));
    }
  }
  const disc = Array.from({ length: 22 }, (_, index): Point => {
    const angle = (index / 22) * Math.PI * 2;
    return [Math.cos(angle) * 58, Math.sin(angle) * 58];
  });
  const sunPath = smoothPath(wobble(disc, 1.2 * theme.shapeWobble, random, 40), true);
  if (!theme.props.sunFace) {
    // A plain disc with no rays or face: the sky, not a character.
    return fillPath(theme, sunPath, theme.color("#ffd95e"), { strokeWidth: 1.6 * theme.line.weight });
  }
  parts.push(`<path d="${sunPath}" fill="${theme.color("#ffd95e")}" stroke="${theme.line.color}" stroke-width="1.6"/>`);
  parts.push(`<ellipse cx="-20" cy="-8" rx="4" ry="5.5" fill="${theme.line.color}"/><ellipse cx="20" cy="-8" rx="4" ry="5.5" fill="${theme.line.color}"/>`);
  parts.push(`<ellipse cx="-33" cy="11" rx="9" ry="5" fill="#ff8a6c" fill-opacity="0.55"/><ellipse cx="33" cy="11" rx="9" ry="5" fill="#ff8a6c" fill-opacity="0.55"/>`);
  parts.push(`<path d="M-17 13Q0 32 17 13" fill="none" stroke="${theme.line.color}" stroke-width="2.6" stroke-linecap="round"/>`);
  return parts.join("\n");
}

export function drawCloud(theme: Theme, puffs: readonly (readonly [number, number, number])[], seed: string): string {
  const random = seededRandom(seed);
  if (theme.props.clouds === "streaks") {
    // Long flat bars across the puffs' extent, a larger one with a shorter one tucked below.
    const left = Math.min(...puffs.map(([x, , radius]) => x - radius));
    const right = Math.max(...puffs.map(([x, , radius]) => x + radius));
    const thickness = Math.max(...puffs.map(([, , radius]) => radius)) * 0.62;
    const bar = (start: number, end: number, centerY: number, height: number): string =>
      `<rect x="${formatNumber(start)}" y="${formatNumber(centerY - height / 2)}" width="${formatNumber(end - start)}" height="${formatNumber(height)}" rx="${formatNumber(height / 2)}" fill="${theme.color("#fff8ee")}"/>`;
    const span = right - left;
    return bar(left + span * 0.08, right, -thickness * 0.35, thickness) + bar(left, left + span * 0.62, thickness * 0.5, thickness * 0.62);
  }
  const shape = (extra: number, fill: string): string =>
    puffs
      .map(([x, y, radius]) => {
        const points = Array.from({ length: 12 }, (_, index): Point => {
          const angle = (index / 12) * Math.PI * 2;
          return [x + Math.cos(angle) * (radius + extra), y + Math.sin(angle) * (radius + extra)];
        });
        return `<path d="${smoothPath(wobble(points, 0.5 * theme.shapeWobble, random, 20), true)}" fill="${fill}"/>`;
      })
      .join("");
  return shape(1.3 * theme.line.weight, theme.line.color) + shape(0, theme.color("#fff8ee"));
}
