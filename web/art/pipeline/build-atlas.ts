// Rasterizes every SVG under web/art/drawings/ into sprite atlas pages and atlas.json.
//
// A drawing is found by its path alone: drawings/<category>/<sprite>/<view>/<animation>_<frame>.svg
// Its root <svg> needs pixel width and height at scale 1, plus data-anchor-x and data-anchor-y, the
// point that stands on the ground. Any other data-* attributes are copied to the frame's properties.
// So replacing or adding a drawing needs no code changes: drop in the file and run `make art`.
import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { CAST, personAppearance, type PersonAppearance } from "../generators/people.ts";
import { COLORS } from "../generators/palette.ts";
import { themeNamed } from "../generators/themes.ts";

const ART_DIRECTORY = join(dirname(fileURLToPath(import.meta.url)), "..");
// 1x and 2x cover normal play; 3x keeps the closest camera zoom sharp without resampling.
const SCALES = [1, 2, 3] as const;
const PADDING_PIXELS = 4;
const BASE_PAGE_WIDTH = 2048;
const MAX_PAGE_SIZE = 4096;
const DRAWING_PATH = /^([^/]+)\/([^/]+)\/([^/]+)\/([a-z]+)_(\d+)\.svg$/;

interface Drawing {
  key: string;
  source: string;
  svg: string;
  sprite: string;
  view: string;
  animation: string;
  frame: number;
  width: number;
  height: number;
  anchor: { x: number; y: number };
  properties: Record<string, string>;
}
interface Placement { page: number; x: number; y: number; width: number; height: number }
interface Page { file: string; width: number; height: number }

export async function replaceDirectoryWithStagedOutput(
  outputDirectory: string,
  generate: (stagingDirectory: string) => Promise<void>,
): Promise<void> {
  const parentDirectory = dirname(outputDirectory);
  const stagingDirectory = join(parentDirectory, `.${randomUUID()}.staging`);
  const backupDirectory = join(parentDirectory, `.${randomUUID()}.backup`);
  mkdirSync(parentDirectory, { recursive: true });
  mkdirSync(stagingDirectory);
  try {
    await generate(stagingDirectory);
  } catch (error) {
    rmSync(stagingDirectory, { recursive: true, force: true });
    throw error;
  }

  let previousOutputMoved = false;
  try {
    if (existsSync(outputDirectory)) {
      renameSync(outputDirectory, backupDirectory);
      previousOutputMoved = true;
    }
    renameSync(stagingDirectory, outputDirectory);
  } catch (error) {
    if (previousOutputMoved) renameSync(backupDirectory, outputDirectory);
    rmSync(stagingDirectory, { recursive: true, force: true });
    throw error;
  }
  if (previousOutputMoved) rmSync(backupDirectory, { recursive: true, force: true });
}

function listSvgFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return listSvgFiles(path);
    return entry.name.endsWith(".svg") ? [path] : [];
  });
}

function readDrawing(path: string, drawingsDirectory: string): Drawing {
  const source = relative(drawingsDirectory, path).split(sep).join("/");
  const match = DRAWING_PATH.exec(source);
  if (!match) throw new Error(`${source}: expected <category>/<sprite>/<view>/<animation>_<frame>.svg`);
  const [, category, spriteName, view, animation, frameText] = match;
  const svg = readFileSync(path, "utf8");
  const rootTag = /<svg\b[^>]*>/.exec(svg)?.[0];
  if (!rootTag) throw new Error(`${source}: no <svg> root element`);
  const attributes = Object.fromEntries([...rootTag.matchAll(/([\w:-]+)="([^"]*)"/g)].map(([, name, value]) => [name, value]));
  const numberAttribute = (name: string): number => {
    const value = Number(attributes[name]);
    if (attributes[name] === undefined || !Number.isFinite(value)) throw new Error(`${source}: root <svg> needs a numeric ${name}`);
    return value;
  };
  const properties = Object.fromEntries(
    Object.entries(attributes)
      .filter(([name]) => name.startsWith("data-") && !name.startsWith("data-anchor-"))
      .map(([name, value]) => [name.slice("data-".length), value]),
  );
  const sprite = `${category}/${spriteName}`;
  return {
    key: `${sprite}/${view}/${animation}/${Number(frameText)}`,
    source,
    svg,
    sprite,
    view,
    animation,
    frame: Number(frameText),
    width: numberAttribute("width"),
    height: numberAttribute("height"),
    anchor: { x: numberAttribute("data-anchor-x"), y: numberAttribute("data-anchor-y") },
    properties,
  };
}

/** Shelf packing: tallest first, new shelf when a row is full, new page when a page is full. */
function packShelves(drawings: readonly Drawing[], scale: number): { placements: Map<string, Placement>; sizes: { width: number; height: number }[] } {
  const pageWidth = Math.min(MAX_PAGE_SIZE, BASE_PAGE_WIDTH * scale);
  const placements = new Map<string, Placement>();
  const sizes: { width: number; height: number }[] = [{ width: 0, height: 0 }];
  let [cursorX, cursorY, shelfHeight] = [PADDING_PIXELS, PADDING_PIXELS, 0];
  const ordered = [...drawings].sort((first, second) => second.height - first.height || first.key.localeCompare(second.key));
  for (const drawing of ordered) {
    const [width, height] = [Math.ceil(drawing.width * scale), Math.ceil(drawing.height * scale)];
    if (width + PADDING_PIXELS * 2 > pageWidth) throw new Error(`${drawing.source}: too wide for a ${pageWidth}px atlas page`);
    if (cursorX + width + PADDING_PIXELS > pageWidth) {
      cursorX = PADDING_PIXELS;
      cursorY += shelfHeight + PADDING_PIXELS;
      shelfHeight = 0;
    }
    if (cursorY + height + PADDING_PIXELS > MAX_PAGE_SIZE) {
      sizes.push({ width: 0, height: 0 });
      [cursorX, cursorY, shelfHeight] = [PADDING_PIXELS, PADDING_PIXELS, 0];
    }
    const page = sizes.length - 1;
    placements.set(drawing.key, { page, x: cursorX, y: cursorY, width, height });
    sizes[page].width = Math.max(sizes[page].width, cursorX + width + PADDING_PIXELS);
    sizes[page].height = Math.max(sizes[page].height, cursorY + height + PADDING_PIXELS);
    cursorX += width + PADDING_PIXELS;
    shelfHeight = Math.max(shelfHeight, height);
  }
  return { placements, sizes };
}

function buildSpriteIndex(drawings: readonly Drawing[]): Record<string, { views: Record<string, Record<string, string[]>>; appearance?: PersonAppearance }> {
  const appearanceBySprite = new Map(CAST.map((person) => [`people/${person.id}`, personAppearance(person)]));
  const sprites: Record<string, { views: Record<string, Record<string, string[]>>; appearance?: PersonAppearance }> = {};
  const sorted = [...drawings].sort((first, second) => first.frame - second.frame);
  for (const drawing of sorted) {
    const sprite = (sprites[drawing.sprite] ??= { views: {} });
    if (drawing.sprite.startsWith("people/")) {
      const appearance = appearanceBySprite.get(drawing.sprite);
      if (!appearance) throw new Error(`${drawing.sprite}: no matching person design in CAST`);
      sprite.appearance = appearance;
    }
    const views = sprite.views;
    const frames = ((views[drawing.view] ??= {})[drawing.animation] ??= []);
    if (drawing.frame !== frames.length) throw new Error(`${drawing.source}: frame ${frames.length} of this animation is missing`);
    frames.push(drawing.key);
  }
  return sprites;
}

async function main(): Promise<void> {
  const [drawingsArgument, outputArgument, themeArgument] = process.argv.slice(2);
  const drawingsDirectory = drawingsArgument ?? join(ART_DIRECTORY, "drawings");
  const outputDirectory = outputArgument ?? join(ART_DIRECTORY, "atlas");
  const theme = themeNamed(themeArgument ?? "print-modern");
  const drawings = listSvgFiles(drawingsDirectory).map((path) => readDrawing(path, drawingsDirectory));
  if (drawings.length === 0) throw new Error(`No drawings found under ${drawingsDirectory}`);
  const sprites = buildSpriteIndex(drawings);

  await replaceDirectoryWithStagedOutput(outputDirectory, async (stagingDirectory) => {
    const browser = await chromium.launch({ channel: "chrome" });
    const browserPage = await browser.newPage();

    const pagesByScale: Record<string, Page[]> = {};
    const placementsByKey: Record<string, Record<string, Placement>> = {};
    for (const scale of SCALES) {
      const { placements, sizes } = packShelves(drawings, scale);
      pagesByScale[scale] = [];
      for (const [pageIndex, size] of sizes.entries()) {
        const onPage = drawings
          .filter((drawing) => placements.get(drawing.key)?.page === pageIndex)
          .map((drawing) => ({ source: drawing.source, svg: drawing.svg, placement: placements.get(drawing.key) }));
        // Chrome rasterizes an SVG image at its drawn size, so each scale is sharp rather than resampled.
        const { dataUrl, clipped } = await browserPage.evaluate(
          async ({ items, width, height }) => {
            const atlas = document.createElement("canvas");
            atlas.width = width;
            atlas.height = height;
            const context = atlas.getContext("2d", { willReadFrequently: true })!;
            const clippedSources: string[] = [];
            for (const { source, svg, placement } of items) {
              const image = new Image();
              image.src = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(svg)))}`;
              await image.decode();
              context.drawImage(image, placement!.x, placement!.y, placement!.width, placement!.height);
              const { x, y, width: w, height: h } = placement!;
              const rows = [context.getImageData(x, y, w, 1), context.getImageData(x, y + h - 1, w, 1), context.getImageData(x, y, 1, h), context.getImageData(x + w - 1, y, 1, h)];
              if (rows.some(({ data }) => data.some((value, index) => index % 4 === 3 && value > 8))) clippedSources.push(source);
            }
            return { dataUrl: atlas.toDataURL("image/png"), clipped: clippedSources };
          },
          { items: onPage, width: size.width, height: size.height },
        );
        if (clipped.length > 0) throw new Error(`Drawing reaches the edge of its canvas, so it is cut off:\n${clipped.join("\n")}`);
        const file = `atlas@${scale}x_${pageIndex}.png`;
        writeFileSync(join(stagingDirectory, file), Buffer.from(dataUrl.split(",")[1], "base64"));
        pagesByScale[scale].push({ file, width: size.width, height: size.height });
      }
      for (const drawing of drawings) (placementsByKey[drawing.key] ??= {})[scale] = placements.get(drawing.key)!;
    }
    await browser.close();

    const sortedDrawings = [...drawings].sort((first, second) => first.key.localeCompare(second.key));
    const frames = Object.fromEntries(
      sortedDrawings.map((drawing) => [
        drawing.key,
        {
          source: drawing.source,
          sourceHash: createHash("sha256").update(drawing.svg).digest("hex"),
          width: drawing.width,
          height: drawing.height,
          anchor: drawing.anchor,
          properties: drawing.properties,
          placements: placementsByKey[drawing.key],
        },
      ]),
    );
    const themeStyle = {
      steel: theme.color(COLORS.red),
      wood: theme.color(COLORS.wood),
      train: theme.color(COLORS.mint),
      ink: theme.line.color,
      lineWeight: theme.line.weight,
    };
    writeFileSync(
      join(stagingDirectory, "atlas.json"),
      JSON.stringify({ scales: SCALES, padding: PADDING_PIXELS, tile: { width: 64, height: 32 }, themeStyle, pages: pagesByScale, sprites, frames }, null, 1) + "\n",
    );
    const pageSummary = SCALES.map((scale) => `${scale}x: ${pagesByScale[scale].map(({ width, height }) => `${width}x${height}`).join(", ")}`).join("; ");
    console.info(`Packed ${drawings.length} drawings. ${pageSummary}`);
  });
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
