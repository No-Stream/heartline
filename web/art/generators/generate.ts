// Writes the parametric drawings as ordinary SVG files.
// `make art` reads only those files, so any file can be replaced by a hand-made SVG.
// Running this generator again overwrites the drawings it owns.
//
// Usage: generate.ts [theme] [output directory]. The default is the candy theme written to
// web/art/drawings/, which is what the game uses.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { BUILDINGS } from "./buildings.ts";
import { CLIFF_CANVAS, CLIFF_STEP_CANVAS, drawCliff, drawCloud, drawGrass, drawPath, drawSun, drawWater, HEIGHT_STEP_PIXELS, TILE_CANVAS } from "./ground.ts";
import { svgDocument } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { CAST, type Facing, IDLE_FRAME_COUNT, personDocument, WALK_FRAME_COUNT } from "./people.ts";
import { sceneryDesigns } from "./scenery.ts";
import { themeNamed } from "./themes.ts";

const [themeArgument, outputArgument] = process.argv.slice(2);
const theme = themeNamed(themeArgument ?? "candy");
const DRAWINGS_DIRECTORY = outputArgument ? resolve(outputArgument) : join(dirname(fileURLToPath(import.meta.url)), "..", "drawings");
const FACINGS: readonly Facing[] = ["se", "sw", "ne", "nw"];
const ROTATION_NAMES = ["rot0", "rot90", "rot180", "rot270"];

function writeDrawing(relativePath: string, svg: string): void {
  const path = join(DRAWINGS_DIRECTORY, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, svg);
}

function placed(width: number, height: number, anchorX: number, anchorY: number, body: string, extras: Record<string, string | number> = {}): string {
  return svgDocument(width, height, anchorX, anchorY, `<g transform="translate(${anchorX} ${anchorY})">\n${body}\n</g>`, extras, theme);
}

for (const person of CAST) {
  for (const facing of FACINGS) {
    for (let frame = 0; frame < WALK_FRAME_COUNT; frame++) {
      writeDrawing(`people/${person.id}/${facing}/walk_${frame}.svg`, personDocument(person, { facing, animation: "walk", frame }, theme));
    }
    for (let frame = 0; frame < IDLE_FRAME_COUNT; frame++) {
      writeDrawing(`people/${person.id}/${facing}/idle_${frame}.svg`, personDocument(person, { facing, animation: "idle", frame }, theme));
    }
  }
}

for (const building of BUILDINGS) {
  ROTATION_NAMES.forEach((rotationName, view) => {
    writeDrawing(
      `buildings/${building.id}/${rotationName}/static_0.svg`,
      placed(building.canvasWidth, building.canvasHeight, building.canvasWidth / 2, building.anchorY, building.draw(theme, view), {
        "data-description": building.description,
        "data-footprint-x": building.footprintX,
        "data-footprint-y": building.footprintY,
      }),
    );
  });
}

for (const item of sceneryDesigns(theme)) {
  const views = item.rotates ? ROTATION_NAMES : ["any"];
  views.forEach((viewName, view) => {
    for (let frame = 0; frame < item.frames; frame++) {
      const animation = item.frames > 1 ? "loop" : "static";
      writeDrawing(
        `scenery/${item.id}/${viewName}/${animation}_${frame}.svg`,
        placed(item.width, item.height, item.width / 2, item.anchorY, item.draw(view, frame), { "data-description": item.description }),
      );
    }
  });
}

const tile = (body: string, description: string, coasterTheme = false): string =>
  placed(TILE_CANVAS.width, TILE_CANVAS.height, TILE_CANVAS.anchorX, TILE_CANVAS.anchorY, body, {
    "data-description": description,
    ...(coasterTheme ? {
      "data-coaster-steel": theme.color(COLORS.red),
      "data-coaster-wood": theme.color(COLORS.wood),
      "data-coaster-train": theme.color(COLORS.mint),
      "data-coaster-ink": theme.line.color,
      "data-coaster-line-weight": theme.line.weight,
    } : {}),
  });

[0, 1, 2].forEach((variant) => {
  writeDrawing(`ground/grass_light_${variant}/any/static_0.svg`, tile(drawGrass(theme, variant, COLORS.grass), "Light grass tile", variant === 0));
  writeDrawing(`ground/grass_dark_${variant}/any/static_0.svg`, tile(drawGrass(theme, variant, COLORS.grassAlternate), "Dark grass tile"));
});
for (let connectionBits = 0; connectionBits < 16; connectionBits++) {
  const connections = [0, 1, 2, 3].map((bit) => Boolean(connectionBits & (1 << bit))) as [boolean, boolean, boolean, boolean];
  const bitsText = connections.map(Number).join("");
  writeDrawing(`ground/path_${bitsText}/any/static_0.svg`, tile(drawPath(theme, connections), `Path tile joined on sides +x +y -x -y = ${bitsText}`));
}
const WATER_FRAME_COUNT = 4;
writeDrawing("ground/water/any/static_0.svg", tile(drawWater(theme, 0, WATER_FRAME_COUNT), "Water tile"));
for (let frame = 0; frame < WATER_FRAME_COUNT; frame++) {
  writeDrawing(`ground/water/any/loop_${frame}.svg`, tile(drawWater(theme, frame, WATER_FRAME_COUNT), "Water tile with drifting ripples"));
}
for (const side of ["right", "left"] as const) {
  [0, 1, 2].forEach((variant) => {
    const anchorX = side === "right" ? CLIFF_STEP_CANVAS.width - CLIFF_STEP_CANVAS.anchorX : CLIFF_STEP_CANVAS.anchorX;
    writeDrawing(
      `ground/cliff_step_${side}_${variant}/any/static_0.svg`,
      placed(CLIFF_STEP_CANVAS.width, CLIFF_STEP_CANVAS.height, anchorX, CLIFF_STEP_CANVAS.anchorY, drawCliff(theme, side, HEIGHT_STEP_PIXELS, variant, 0.6), {
        "data-description": `Earth face one height step tall below a ${side}-hand tile edge`,
      }),
    );
  });
}
for (const side of ["right", "left"] as const) {
  [0, 1, 2].forEach((variant) => {
    const anchorX = side === "right" ? CLIFF_CANVAS.width - 2 : 2;
    writeDrawing(
      `ground/cliff_${side}_${variant}/any/static_0.svg`,
      placed(CLIFF_CANVAS.width, CLIFF_CANVAS.height, anchorX, CLIFF_CANVAS.anchorY, drawCliff(theme, side, 44, variant), { "data-description": `Earth cliff below a ${side}-hand tile edge` }),
    );
  });
}

writeDrawing("sky/sun/any/static_0.svg", placed(224, 224, 112, 112, drawSun(theme), { "data-description": theme.props.sunFace ? "Smiling sun" : "Sun" }));
writeDrawing("sky/cloud_big/any/static_0.svg", placed(160, 84, 80, 48, drawCloud(theme, [[-38, 2, 19], [-14, -10, 26], [16, -6, 23], [42, 4, 17], [2, 8, 20]], "cloud_big"), { "data-description": "Big cloud" }));
writeDrawing("sky/cloud_small/any/static_0.svg", placed(104, 56, 52, 32, drawCloud(theme, [[-22, 1, 13], [0, -6, 17], [23, 2, 12]], "cloud_small"), { "data-description": "Small cloud" }));
