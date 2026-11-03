import { INK, shadePlum } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model, storybookWarp } from "./solid.ts";
import type { Theme } from "./theme.ts";
import { gableRoofGeometry, ribbed, seamedRoof, pictogramSign } from "./building-primitives.ts";

export function drawRestroom(theme: Theme, view: number): string {
  const wallHeight = 40;
  const warp = storybookWarp([-1.6, 2], 0.04, -1.2, 80, theme.crookedness);
  const model = new Model("restroom", view, theme, warp);
  const half = 33;
  model.shadow([[-half - 1, -half - 1, 0], [half + 7, -half - 1, 0], [half + 7, half + 2, 0], [-half - 1, half + 2, 0]]);

  // Back: a little water tank on stilts.
  model.group([-half - 8, 0, 8], -3);
  for (const [legX, legY] of [[-half - 13, -7], [-half - 13, 7], [-half - 3, -7], [-half - 3, 7]]) {
    model.stick([legX, legY, 0], [legX, legY, 22], 1.5, COLORS.woodDark, 0.7);
  }
  model.lathe(-half - 8, 0, [[9, 21], [9.4, 24], [9.4, 34], [6, 37], [1, 38]], COLORS.sky);
  model.lineOn(model.surface([[-half - 3.5, -9, 27], [-half - 3.5, 9, 27], [-half - 3.5, 9, 30], [-half - 3.5, -9, 30]]), [[0, 1], [18, 1]], 0.6, INK, 0.5);

  model.group([0, 0, 20]);
  const walls = model.box(0, 0, 0, half * 2, half * 2, wallHeight, COLORS.mint, { hatch: 6 });
  for (const wall of [walls.front, walls.right, walls.back, walls.left]) {
    if (theme.props.ornaments === "candy") {
      const tileCount = 9;
      for (let index = 0; index < tileCount; index++) {
        const tileWidth = wall.width / tileCount;
        model.rect(wall, index * tileWidth, 0, tileWidth, 5.2, index % 2 ? COLORS.paper : COLORS.leafTeal, 0);
        model.rect(wall, index * tileWidth, 5.2, tileWidth, 5.2, index % 2 ? COLORS.leafTeal : COLORS.paper, 0);
      }
    } else {
      model.rect(wall, 0, 0, wall.width, 10.4, COLORS.leafTeal, 0);
      ribbed(model, wall, 0, 10.4, 11);
    }
    model.lineOn(wall, [[0, 10.4], [wall.width, 10.4]], 1);
  }

  // Front: two doors, with hearts cut in and person-shaped signs, or with pictogram plates.
  const doorWidth = 19;
  for (const [doorLeft, doorColor, iconSkirted] of [[8, COLORS.pink, true], [39, COLORS.blue, false]] as const) {
    model.arch(walls.front, doorLeft, 0, doorWidth, 33, doorColor, 1.2);
    model.ellipseOn(walls.front, doorLeft + 15.8, 14, 1.1, 1.1, COLORS.yellow, 0.6);
    const heartU = doorLeft + doorWidth / 2;
    if (theme.props.ornaments === "plain") {
      pictogramSign(model, walls.front, heartU, 17, iconSkirted, COLORS.plumDark);
      continue;
    }
    model.strokeOn(walls.front, [[heartU, 23], [heartU - 2.5, 25.8], [heartU - 1.2, 27.3], [heartU, 26], [heartU + 1.2, 27.3], [heartU + 2.5, 25.8], [heartU, 23]], 1, COLORS.paper);
    // Person icon on the door.
    model.ellipseOn(walls.front, heartU, 19.4, 1.8, 1.8, COLORS.paper, 0);
    model.paint(walls.front, iconSkirted
      ? [[heartU - 3.2, 8], [heartU + 3.2, 8], [heartU + 1.4, 17], [heartU - 1.4, 17]]
      : [[heartU - 1.6, 8], [heartU + 1.6, 8], [heartU + 2, 17], [heartU - 2, 17]], COLORS.paper, 0);
  }

  // Right and left: high little windows and a flower box. Right also has a drainpipe.
  for (const windowU of [20, 46]) {
    model.arch(walls.right, windowU - 7, 19, 14, 15, COLORS.sky, 1.1);
    model.lineOn(walls.right, [[windowU, 19], [windowU, 33]], 0.6);
  }
  model.rect(walls.right, 60, 0, 2.6, wallHeight, "#c8c2d6", 0.8);
  model.arch(walls.left, 24, 18, 18, 17, COLORS.sky, 1.1);
  model.lineOn(walls.left, [[33, 18], [33, 34]], 0.6);
  model.lineOn(walls.left, [[24, 25], [42, 25]], 0.6);
  model.rect(walls.left, 21, 13, 24, 5.5, COLORS.wood, 1);
  for (const bloomU of [24, 28.5, 33, 37.5, 42]) {
    model.ellipseOn(walls.left, bloomU, 20, 2.2, 2.2, bloomU % 2 ? COLORS.yellow : COLORS.pink, 0.7);
  }
  model.rect(walls.back, 25, 12, 16, 19, shadePlum(COLORS.paper, 0.1), 1);
  model.lineOn(walls.back, [[33, 12], [33, 31]], 0.6);

  // Gable roof with a ridge running from side to side, so the gable ends face front and back.
  const overhang = 6;
  const ridgeHeight = wallHeight + 28;
  const edge = half + overhang;
  model.group([0, 0, wallHeight + 14], 8);
  const roofGeometry = gableRoofGeometry(half, edge, wallHeight, ridgeHeight, 2, 5);
  model.face(roofGeometry.frontGable, COLORS.paper, { ink: 1 });
  model.face(roofGeometry.backGable, COLORS.paper, { ink: 1 });
  const gableFrontSurface = model.surface(roofGeometry.frontSurface);
  model.ellipseOn(gableFrontSurface, half, 11, 7.8, 7.8, COLORS.yellow, 1);
  model.letter(gableFrontSurface, "WC", half, 7.8, 6.2, INK, 1.1);
  const gableBackSurface = model.surface(roofGeometry.backSurface);
  model.ellipseOn(gableBackSurface, half, 10.5, 5, 5, COLORS.sky, 0.9);

  for (const slope of roofGeometry.slopes) {
    model.face(slope, COLORS.violet, { ink: 1.3 });
    if (theme.props.ornaments === "candy") model.shingles(model.surface(slope), 6, 9, INK);
    else seamedRoof(model, model.surface(slope), 9);
  }

  // Vent pipe with a little cap on the back slope.
  model.group([-16, -14, ridgeHeight], 10);
  model.pipe(-16, -14, wallHeight + 16, ridgeHeight + 4, 3.2, COLORS.peach);
  return model.render();
}
