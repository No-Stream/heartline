// Writes the parametric drawings to web/art/drawings/ as ordinary SVG files.
// `make art` reads only those files, so any file can be replaced by a hand-made SVG.
// Running this generator again overwrites the drawings it owns.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { BUILDINGS } from "./buildings.ts";
import { CLIFF_CANVAS, drawCliff, drawCloud, drawGrass, drawPath, drawSun, TILE_CANVAS } from "./ground.ts";
import { svgDocument } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { CAST, type Facing, IDLE_FRAME_COUNT, personDocument, WALK_FRAME_COUNT } from "./people.ts";
import { SCENERY } from "./scenery.ts";

const DRAWINGS_DIRECTORY = join(dirname(fileURLToPath(import.meta.url)), "..", "drawings");
const FACINGS: readonly Facing[] = ["se", "sw", "ne", "nw"];
const ROTATION_NAMES = ["rot0", "rot90", "rot180", "rot270"];

function writeDrawing(relativePath: string, svg: string): void {
  const path = join(DRAWINGS_DIRECTORY, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, svg);
}

function placed(width: number, height: number, anchorX: number, anchorY: number, body: string, extras: Record<string, string | number> = {}): string {
  return svgDocument(width, height, anchorX, anchorY, `<g transform="translate(${anchorX} ${anchorY})">\n${body}\n</g>`, extras);
}

for (const person of CAST) {
  for (const facing of FACINGS) {
    for (let frame = 0; frame < WALK_FRAME_COUNT; frame++) {
      writeDrawing(`people/${person.id}/${facing}/walk_${frame}.svg`, personDocument(person, { facing, animation: "walk", frame }));
    }
    for (let frame = 0; frame < IDLE_FRAME_COUNT; frame++) {
      writeDrawing(`people/${person.id}/${facing}/idle_${frame}.svg`, personDocument(person, { facing, animation: "idle", frame }));
    }
  }
}

for (const building of BUILDINGS) {
  ROTATION_NAMES.forEach((rotationName, view) => {
    writeDrawing(
      `buildings/${building.id}/${rotationName}/static_0.svg`,
      placed(building.canvasWidth, building.canvasHeight, building.canvasWidth / 2, building.anchorY, building.draw(view), {
        "data-description": building.description,
        "data-footprint-x": building.footprintX,
        "data-footprint-y": building.footprintY,
      }),
    );
  });
}

for (const item of SCENERY) {
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

const tile = (body: string, description: string): string =>
  placed(TILE_CANVAS.width, TILE_CANVAS.height, TILE_CANVAS.anchorX, TILE_CANVAS.anchorY, body, { "data-description": description });

[0, 1, 2].forEach((variant) => {
  writeDrawing(`ground/grass_light_${variant}/any/static_0.svg`, tile(drawGrass(variant, COLORS.grass), "Light grass tile"));
  writeDrawing(`ground/grass_dark_${variant}/any/static_0.svg`, tile(drawGrass(variant, COLORS.grassAlternate), "Dark grass tile"));
});
for (let connectionBits = 0; connectionBits < 16; connectionBits++) {
  const connections = [0, 1, 2, 3].map((bit) => Boolean(connectionBits & (1 << bit))) as [boolean, boolean, boolean, boolean];
  const bitsText = connections.map(Number).join("");
  writeDrawing(`ground/path_${bitsText}/any/static_0.svg`, tile(drawPath(connections), `Path tile joined on sides +x +y -x -y = ${bitsText}`));
}
for (const side of ["right", "left"] as const) {
  [0, 1, 2].forEach((variant) => {
    const anchorX = side === "right" ? CLIFF_CANVAS.width - 2 : 2;
    writeDrawing(
      `ground/cliff_${side}_${variant}/any/static_0.svg`,
      placed(CLIFF_CANVAS.width, CLIFF_CANVAS.height, anchorX, CLIFF_CANVAS.anchorY, drawCliff(side, 44, variant), { "data-description": `Earth cliff below a ${side}-hand tile edge` }),
    );
  });
}

writeDrawing("sky/sun/any/static_0.svg", placed(224, 224, 112, 112, drawSun(), { "data-description": "Smiling sun" }));
writeDrawing("sky/cloud_big/any/static_0.svg", placed(160, 84, 80, 48, drawCloud([[-38, 2, 19], [-14, -10, 26], [16, -6, 23], [42, 4, 17], [2, 8, 20]], "cloud_big"), { "data-description": "Big cloud" }));
writeDrawing("sky/cloud_small/any/static_0.svg", placed(104, 56, 52, 32, drawCloud([[-22, 1, 13], [0, -6, 17], [23, 2, 12]], "cloud_small"), { "data-description": "Small cloud" }));
