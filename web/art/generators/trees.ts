// Tree shapes for the themes other than candy. Candy's lollipop, cloud and pine trees live in scenery.ts.
// All colours are design colours, mapped by the model's theme.
import { fillPath, inkLine, type Point, smoothPath, wobble } from "./ink.ts";
import { COLORS } from "./palette.ts";
import type { Model } from "./solid.ts";
import type { TreeShape } from "./theme.ts";

type CircleSpec = readonly [offsetX: number, offsetY: number, radius: number];

export function drawThemedTree(model: Model, shape: TreeShape): string {
  switch (shape.kind) {
    case "printedBlob":
      drawPrintedBlob(model, shape);
      break;
    case "poplar":
      drawPoplar(model, shape);
      break;
    case "scribble":
      drawScribble(model, shape);
      break;
    case "willowy":
      drawWillowy(model, shape);
      break;
    case "cone":
      drawCone(model, shape);
      break;
    case "oval":
      drawOval(model, shape);
      break;
    case "tiers":
      drawTiers(model, shape);
      break;
    case "cypress":
      drawCypress(model, shape);
      break;
    default:
      throw new Error(`${shape.kind} trees are drawn in scenery.ts`);
  }
  return model.render();
}

/** Circles laid out as a rounded crown: one in the middle and a ring of lobes. */
function crownCircles(radius: number, lobes: number, phase: number): CircleSpec[] {
  const ring = Array.from({ length: lobes }, (_, index): CircleSpec => {
    const angle = phase + (index / lobes) * Math.PI * 2;
    return [Math.cos(angle) * radius * 0.52, Math.sin(angle) * radius * 0.44, radius * (0.42 + 0.06 * Math.sin(index * 2.3))];
  });
  return [[0, 0, radius * 0.68], ...ring];
}

/** A crown printed with two inks: the second plate is offset towards the light and prints over the first. */
function drawPrintedBlob(model: Model, shape: Extract<TreeShape, { kind: "printedBlob" }>): void {
  const { crownRadius, trunkHeight, lobes } = shape;
  model.groundEllipse(crownRadius * 0.7, crownRadius * 0.7, 0.26, 3, 3);
  model.tube([[0, 0, 0], [0.6, -0.4, trunkHeight * 0.55], [0, 0, trunkHeight + crownRadius * 0.6]], 2.8, COLORS.trunk, 0.8);
  model.tube([[0.3, -0.2, trunkHeight * 0.7], [4, -4, trunkHeight + crownRadius * 0.2]], 1.3, COLORS.trunk, 0.5);
  const [centerX, centerY] = model.toScreen([0, 0, trunkHeight + crownRadius * 0.85]);
  for (const [offsetX, offsetY, radius] of crownCircles(crownRadius, lobes, 0.3)) {
    model.raw(model.blob(centerX + offsetX, centerY + offsetY, radius, shape.first, 0));
  }
  // A lighter second plate, so the overlap is a mid green rather than near black.
  model.raw(`<g style="mix-blend-mode:multiply" opacity="0.72">`);
  const [shiftX, shiftY] = [crownRadius * 0.24, -crownRadius * 0.2];
  for (const [offsetX, offsetY, radius] of crownCircles(crownRadius * 0.8, Math.max(3, lobes - 1), 1.1)) {
    model.raw(model.blob(centerX + shiftX + offsetX, centerY + shiftY + offsetY, radius, shape.second, 0));
  }
  model.raw(`</g>`);
}

function drawPoplar(model: Model, shape: Extract<TreeShape, { kind: "poplar" }>): void {
  const { height, width } = shape;
  model.groundEllipse(width * 0.8, width * 0.8, 0.26, 3, 3);
  model.tube([[0, 0, 0], [0, 0, 14]], 2.6, COLORS.trunk, 0.8);
  const outline = (halfWidth: number, bottom: number, top: number, shiftX: number): Point[] => {
    const [baseX, baseY] = model.toScreen([0, 0, bottom]);
    const span = top - bottom;
    return Array.from({ length: 24 }, (_, index): Point => {
      const angle = (index / 24) * Math.PI * 2;
      const up = (1 - Math.cos(angle)) / 2;
      // Widest a third of the way up, tapering to a soft point.
      const profile = Math.sin(Math.PI * up ** 0.75) * halfWidth;
      return [baseX + shiftX + Math.sin(angle) * profile, baseY - up * span];
    });
  };
  const random = model.randomSource;
  const shapeWobble = model.theme.shapeWobble;
  const first = smoothPath(wobble(outline(width, 8, 8 + height, 0), 0.6 * shapeWobble, random), true);
  model.raw(fillPath(model.theme, first, model.color(shape.color)));
  const second = smoothPath(wobble(outline(width * 0.72, 14, 8 + height * 0.94, width * 0.4), 0.6 * shapeWobble, random), true);
  model.raw(`<g style="mix-blend-mode:multiply" opacity="0.72">${fillPath(model.theme, second, model.color(shape.overprint))}</g>`);
}

/** Points round a loose, lumpy crown, as drawn quickly with a pen. */
function lumpyOutline(centerX: number, centerY: number, radius: number, random: () => number, lumps: number): Point[] {
  const count = lumps * 5;
  const phases = Array.from({ length: lumps }, () => random() * 0.5);
  return Array.from({ length: count }, (_, index): Point => {
    const angle = (index / count) * Math.PI * 2;
    const lump = Math.abs(Math.sin((angle * lumps) / 2 + phases[index % lumps]));
    const reach = radius * (0.86 + 0.16 * lump);
    return [centerX + Math.cos(angle) * reach, centerY + Math.sin(angle) * reach * 0.82];
  });
}

/** A pen-drawn crown: paper inside, a wash that covers only part of it, a loose outline and a few leaf ticks. */
function drawScribble(model: Model, shape: Extract<TreeShape, { kind: "scribble" }>): void {
  const { crownRadius, trunkHeight, lean } = shape;
  const random = model.randomSource;
  const { theme } = model;
  model.groundEllipse(crownRadius * 0.7, crownRadius * 0.6, 0.2, 3, 3);
  const top: [number, number, number] = [lean, -lean, trunkHeight + crownRadius * 0.6];
  const trunk = [model.toScreen([0, 0, 0]), model.toScreen([lean * 0.3, -lean * 0.3, trunkHeight * 0.5]), model.toScreen(top)];
  model.raw(inkLine(trunk, random, { width: model.lineWidth(2.6), color: model.color(COLORS.trunk), wobble: theme.line.wobble }));
  model.raw(inkLine([model.toScreen([lean * 0.4, -lean * 0.4, trunkHeight * 0.65]), model.toScreen([lean + 5, -lean - 5, trunkHeight + crownRadius * 0.3])], random, { width: model.lineWidth(1.2), color: model.ink, wobble: theme.line.wobble }));
  const [centerX, centerY] = model.toScreen([lean, -lean, trunkHeight + crownRadius * 0.85]);
  const outline = lumpyOutline(centerX, centerY, crownRadius, random, 7);
  const crownPath = smoothPath(outline, true, 0.8);
  model.raw(`<path d="${crownPath}" fill="${theme.paper}"/>`);
  const washOutline = lumpyOutline(centerX - crownRadius * 0.12, centerY + crownRadius * 0.1, crownRadius * 0.82, random, 5);
  model.raw(`<path d="${smoothPath(washOutline, true, 0.8)}" fill="${model.color(shape.color)}" filter="url(#wash)"/>`);
  model.raw(inkLine([...outline, outline[0], outline[1]], random, { width: model.lineWidth(1.1), color: model.ink, wobble: theme.line.wobble, breaks: theme.line.breaks, overshoot: 1.5 }));
  // Quick diagonal hatching on the shaded lower left of the crown.
  for (let stroke = 0; stroke < 5; stroke++) {
    const offset = (stroke - 2) * crownRadius * 0.16;
    const [startX, startY] = [centerX - crownRadius * 0.55 + offset, centerY + crownRadius * 0.15 + offset * 0.6];
    const length = crownRadius * (0.32 + random() * 0.18);
    model.raw(inkLine([[startX, startY], [startX + length * 0.7, startY - length * 0.7]], random, { width: model.lineWidth(0.7), color: model.ink, wobble: theme.line.wobble, opacity: 0.8 }));
  }
}

/** A tall, thin tree drawn with a loose outline and a few vertical pen strokes. */
function drawWillowy(model: Model, shape: Extract<TreeShape, { kind: "willowy" }>): void {
  const random = model.randomSource;
  const { theme } = model;
  model.groundEllipse(8, 7, 0.2, 3, 3);
  model.raw(inkLine([model.toScreen([0, 0, 0]), model.toScreen([0, 0, 18])], random, { width: model.lineWidth(2.2), color: model.color(COLORS.trunk), wobble: theme.line.wobble }));
  const [baseX, baseY] = model.toScreen([0, 0, 12]);
  const halfWidth = 9;
  const outline: Point[] = Array.from({ length: 26 }, (_, index) => {
    const angle = (index / 26) * Math.PI * 2;
    const up = (1 - Math.cos(angle)) / 2;
    const profile = Math.sin(Math.PI * up ** 0.7) * halfWidth * (0.9 + random() * 0.2);
    return [baseX + Math.sin(angle) * profile, baseY - up * shape.height];
  });
  const crownPath = smoothPath(outline, true, 0.8);
  model.raw(`<path d="${crownPath}" fill="${theme.paper}"/>`);
  const washPath = smoothPath(outline.map(([x, y]): Point => [baseX + (x - baseX) * 0.75 - 1.5, baseY + (y - baseY) * 0.82]), true, 0.8);
  model.raw(`<path d="${washPath}" fill="${model.color(shape.color)}" filter="url(#wash)"/>`);
  model.raw(inkLine([...outline, outline[0], outline[1]], random, { width: model.lineWidth(1), color: model.ink, wobble: theme.line.wobble, breaks: theme.line.breaks, overshoot: 1.5 }));
  for (let stroke = 0; stroke < 5; stroke++) {
    const strokeX = baseX - halfWidth * 0.5 + stroke * halfWidth * 0.22 + (random() - 0.5);
    const strokeBottom = baseY - shape.height * (0.12 + random() * 0.1);
    const strokeTop = strokeBottom - shape.height * (0.25 + random() * 0.3);
    model.raw(inkLine([[strokeX, strokeBottom], [strokeX + 0.6, strokeTop]], random, { width: model.lineWidth(0.7), color: model.ink, wobble: theme.line.wobble }));
  }
}

/** A flat shape split down the middle into a lit right half and a shaded left half. */
function twoTone(model: Model, outline: readonly Point[], splitX: number, color: string): void {
  const themed = model.color(color);
  const path = smoothPath(outline, true, 0.35);
  model.raw(fillPath(model.theme, path, themed));
  const xs = outline.map(([x]) => x);
  const ys = outline.map(([, y]) => y);
  const [minX, minY, maxY] = [Math.min(...xs), Math.min(...ys), Math.max(...ys)];
  const clipId = `split${Math.round(model.randomSource() * 1e9)}`;
  model.raw(
    `<clipPath id="${clipId}"><path d="${path}"/></clipPath>` +
      `<rect clip-path="url(#${clipId})" x="${minX - 1}" y="${minY - 1}" width="${splitX - minX + 1}" height="${maxY - minY + 2}" fill="${model.theme.shade(themed, 0.3)}"/>`,
  );
}

function ellipsePoints(centerX: number, centerY: number, radiusX: number, radiusY: number, count = 24): Point[] {
  return Array.from({ length: count }, (_, index): Point => {
    const angle = (index / count) * Math.PI * 2;
    return [centerX + Math.cos(angle) * radiusX, centerY + Math.sin(angle) * radiusY];
  });
}

/** A straight trunk as a thin flat bar, lit on the right. */
function flatTrunk(model: Model, height: number, width: number): void {
  const [bottomX, bottomY] = model.toScreen([0, 0, 0]);
  const [, topY] = model.toScreen([0, 0, height]);
  const trunk = model.color(COLORS.trunk);
  model.raw(`<rect x="${bottomX - width / 2}" y="${topY}" width="${width}" height="${bottomY - topY + 1}" rx="${width / 2}" fill="${trunk}"/>`);
  model.raw(`<rect x="${bottomX - width / 2}" y="${topY}" width="${width / 2}" height="${bottomY - topY + 1}" fill="${model.theme.shade(trunk, 0.3)}"/>`);
}

function drawCone(model: Model, shape: Extract<TreeShape, { kind: "cone" }>): void {
  const { height, radius } = shape;
  model.groundEllipse(radius * 0.9, radius * 0.9, 0.24, 3, 3);
  flatTrunk(model, 8, 3);
  const [baseX, baseY] = model.toScreen([0, 0, 6]);
  const [apexX, apexY] = model.toScreen([0, 0, 6 + height]);
  const halfWidth = radius * Math.SQRT1_2 * 1.4;
  const arc = ellipsePoints(baseX, baseY, halfWidth, halfWidth * 0.5, 24).filter(([, y]) => y >= baseY - 0.01);
  arc.sort((first, second) => second[0] - first[0]);
  twoTone(model, [[apexX, apexY], ...arc], apexX, shape.color);
}

function drawOval(model: Model, shape: Extract<TreeShape, { kind: "oval" }>): void {
  const { height, radius, trunkHeight } = shape;
  model.groundEllipse(radius * 0.8, radius * 0.8, 0.24, 3, 3);
  flatTrunk(model, trunkHeight + height * 0.3, 2.6);
  const [centerX, centerY] = model.toScreen([0, 0, trunkHeight + height / 2]);
  twoTone(model, ellipsePoints(centerX, centerY, radius, height / 2), centerX - radius * 0.15, shape.color);
}

/** Flat discs stacked like the layers of a stone pine, each with a shaded underside. */
function drawTiers(model: Model, shape: Extract<TreeShape, { kind: "tiers" }>): void {
  const { tiers, radius, trunkHeight } = shape;
  model.groundEllipse(radius * 0.8, radius * 0.8, 0.24, 3, 3);
  const spacing = radius * 0.62;
  flatTrunk(model, trunkHeight + spacing * (tiers - 1), 2.6);
  const themed = model.color(shape.color);
  for (let tier = 0; tier < tiers; tier++) {
    const tierRadius = radius * (1 - tier * 0.24);
    const [centerX, centerY] = model.toScreen([0, 0, trunkHeight + tier * spacing]);
    const underside = smoothPath(ellipsePoints(centerX, centerY + tierRadius * 0.2, tierRadius, tierRadius * 0.42), true);
    model.raw(fillPath(model.theme, underside, model.theme.shade(themed, 0.32)));
    const top = smoothPath(ellipsePoints(centerX, centerY, tierRadius, tierRadius * 0.42), true);
    model.raw(fillPath(model.theme, top, themed));
  }
}

function drawCypress(model: Model, shape: Extract<TreeShape, { kind: "cypress" }>): void {
  const { height, radius } = shape;
  model.groundEllipse(radius, radius, 0.24, 3, 3);
  flatTrunk(model, 8, 2.6);
  const [baseX, baseY] = model.toScreen([0, 0, 5]);
  // A flame: round at the bottom, drawn to a fine point at the top.
  const outline: Point[] = Array.from({ length: 28 }, (_, index) => {
    const angle = (index / 28) * Math.PI * 2;
    const up = (1 - Math.cos(angle)) / 2;
    const profile = Math.sin(Math.PI * up ** 0.6) * radius;
    return [baseX + Math.sin(angle) * profile, baseY - up * height];
  });
  twoTone(model, outline, baseX, shape.color);
}
