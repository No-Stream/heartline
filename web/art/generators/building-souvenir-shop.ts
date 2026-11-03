import { type Vector3 } from "./iso.ts";
import { INK, shadePlum } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model, storybookWarp } from "./solid.ts";
import type { Theme } from "./theme.ts";
import { awningOnFront, gableRoofGeometry, seamedRoof } from "./building-primitives.ts";

export function drawSouvenirShop(theme: Theme, view: number): string {
  const groundHeight = 40;
  const upperHeight = 30;
  const warp = storybookWarp([3.5, -3], 0.02, 2.4, 130, theme.crookedness);
  const model = new Model("souvenir_shop", view, theme, warp);
  const half = 50;
  model.shadow([[-half - 2, -half - 2, 0], [half + 9, -half - 2, 0], [half + 9, half + 3, 0], [-half - 2, half + 3, 0]]);

  // Back: crates and a drainpipe.
  model.group([-half - 7, 22, 0], -3);
  model.box(-half - 7, 22, 0, 10, 16, 11, COLORS.wood, { ink: 1 });
  model.group([-half - 7, 4, 0], -3);
  model.box(-half - 7, 4, 0, 10, 12, 8, shadePlum(COLORS.wood, 0.1), { ink: 1 });

  // Ground floor in warm peach with a timber base.
  model.group([0, 0, groundHeight / 2]);
  const ground = model.box(0, 0, 0, half * 2, half * 2, groundHeight, COLORS.peach, { hatch: 9 });
  for (const wall of [ground.front, ground.right, ground.back, ground.left]) {
    model.rect(wall, 0, 0, wall.width, 6, COLORS.woodDark, 0);
    model.lineOn(wall, [[0, 6], [wall.width, 6]], 1);
  }

  // Front: door in the middle with a big shop window either side.
  const front = ground.front;
  model.rect(front, 8, 8, 30, 25, COLORS.sky, 1.2);
  model.rect(front, 62, 8, 30, 25, COLORS.sky, 1.2);
  model.arch(front, 41, 0, 18, 34, COLORS.red, 1.2);
  model.ellipseOn(front, 55, 15, 1.2, 1.2, COLORS.yellow, 0.6);
  model.ellipseOn(front, 50, 25, 3.8, 3.8, COLORS.sky, 0.8);
  if (front.visible) {
    // Shop window contents: a teddy bear, and a shelf of balls and balloons.
    model.ellipseOn(front, 17, 15.5, 5, 5.6, COLORS.wood, 0.8);
    model.ellipseOn(front, 17, 23.5, 4.3, 4.3, COLORS.wood, 0.8);
    model.ellipseOn(front, 13.4, 27, 1.8, 1.8, COLORS.wood, 0.7);
    model.ellipseOn(front, 20.6, 27, 1.8, 1.8, COLORS.wood, 0.7);
    model.ellipseOn(front, 15.5, 24, 0.6, 0.7, INK, 0);
    model.ellipseOn(front, 18.5, 24, 0.6, 0.7, INK, 0);
    model.ellipseOn(front, 29, 15, 3.8, 3.8, COLORS.pink, 0.8);
    model.ellipseOn(front, 29, 24, 3.6, 4.2, COLORS.yellow, 0.8);
    model.ellipseOn(front, 70, 22, 4, 4.8, COLORS.red, 0.8);
    model.ellipseOn(front, 79, 26, 3.6, 4.2, COLORS.blue, 0.8);
    model.ellipseOn(front, 84, 18, 3.4, 4, COLORS.green, 0.8);
    model.rect(front, 66, 8.5, 22, 5, COLORS.yellow, 0.7);
    model.lineOn(front, [[23, 8], [23, 33]], 0.8);
    model.lineOn(front, [[77, 8], [77, 33]], 0.8);
  }

  // Right: a window and a hanging shop sign. Left: a door-sized lattice with climbing flowers.
  model.rect(ground.right, 30, 13, 40, 19, COLORS.sky, 1.2);
  model.lineOn(ground.right, [[50, 13], [50, 32]], 0.8);
  model.rect(ground.right, 26, 9, 48, 4, COLORS.wood, 1);
  model.rect(ground.left, 28, 6, 44, 30, COLORS.wood, 1);
  if (ground.left.visible) {
    for (let bar = 1; bar < 6; bar++) model.lineOn(ground.left, [[28 + (bar * 44) / 6, 6], [28 + (bar * 44) / 6, 36]], 0.7);
    for (let bar = 1; bar < 4; bar++) model.lineOn(ground.left, [[28, 6 + (bar * 30) / 4], [72, 6 + (bar * 30) / 4]], 0.7);
    for (const [bloomU, bloomV, color] of [[33, 15, COLORS.pink], [45, 24, COLORS.yellow], [39, 31, COLORS.red], [58, 13, COLORS.purple], [66, 27, COLORS.pink], [53, 32, COLORS.yellow]] as const) {
      model.ellipseOn(ground.left, bloomU, bloomV, 3.2, 3.2, color, 0.8);
    }
  }
  model.arch(ground.back, 39, 0, 22, 32, COLORS.blue, 1.2);
  model.rect(ground.back, 12, 14, 18, 15, COLORS.sky, 1.1);
  model.rect(ground.back, 70, 14, 18, 15, COLORS.sky, 1.1);

  // Awning over the front door and windows.
  model.group([half + 6, 0, groundHeight - 3], 8);
  awningOnFront(model, half + 0.5, 0, half * 2 - 4, groundHeight + 1, 13, 8, 11, COLORS.purple, COLORS.paper);

  // Upper floor overhangs slightly and is half-timbered. A storybook detail: beams at odd angles.
  const upperHalf = half + 4;
  model.group([0, 0, groundHeight + upperHeight / 2], 3);
  const upper = model.box(0, 0, groundHeight, upperHalf * 2, upperHalf * 2, upperHeight, COLORS.cream, { hatch: 9 });
  for (const wall of [upper.front, upper.right, upper.back, upper.left]) {
    const beamColor = COLORS.woodDark;
    model.rect(wall, 0, 0, wall.width, 2.8, beamColor, 0.8);
    model.rect(wall, 0, 27, wall.width, 3, beamColor, 0.8);
    model.rect(wall, 0, 0, 3, 30, beamColor, 0.8);
    model.rect(wall, wall.width - 3, 0, 3, 30, beamColor, 0.8);
    model.paint(wall, [[3, 3], [6, 3], [23, 27], [20, 27]], beamColor, 0.7);
    model.paint(wall, [[wall.width - 6, 3], [wall.width - 3, 3], [wall.width - 20, 27], [wall.width - 23, 27]], beamColor, 0.7);
  }
  // Big hand-painted sign on the front; rounded windows with shutters on the other sides.
  model.rect(upper.front, 22, 6, 64, 18, COLORS.yellow, 1.3);
  model.letter(upper.front, "GIFTS", 54, 9.5, 10, COLORS.violet, 1.7);
  for (const wall of [upper.right, upper.left, upper.back]) {
    model.arch(wall, wall.width / 2 - 12, 5, 24, 19, COLORS.sky, 1.2);
    model.lineOn(wall, [[wall.width / 2, 5], [wall.width / 2, 22]], 0.8);
    model.rect(wall, wall.width / 2 - 21, 5, 8, 18, wall === upper.back ? COLORS.leafTeal : COLORS.pink, 0.9);
    model.rect(wall, wall.width / 2 + 13, 5, 8, 18, wall === upper.back ? COLORS.leafTeal : COLORS.pink, 0.9);
  }

  // Steep gable roof, with the ridge running from side to side and gable ends front and back.
  const eaveHeight = groundHeight + upperHeight;
  const ridgeHeight = eaveHeight + 44;
  const edge = upperHalf + 6;
  model.group([0, 0, eaveHeight + 20], 10);
  const roofGeometry = gableRoofGeometry(upperHalf, edge, eaveHeight, ridgeHeight, 3, 6);
  model.face(roofGeometry.frontGable, COLORS.pink, { ink: 1.1 });
  model.face(roofGeometry.backGable, COLORS.pink, { ink: 1.1 });
  const gableFrontSurface = model.surface(roofGeometry.frontSurface);
  model.ellipseOn(gableFrontSurface, upperHalf, 15, 10, 10, COLORS.yellow, 1.1);
  if (theme.props.ornaments === "candy") {
    model.strokeOn(gableFrontSurface, [[upperHalf, 8.5], [upperHalf - 7, 16], [upperHalf - 4, 21], [upperHalf, 18], [upperHalf + 4, 21], [upperHalf + 7, 16], [upperHalf, 8.5]], 1.4, COLORS.red);
  } else {
    // A wrapped present: a box with a ribbon across it and a bow on top.
    model.rect(gableFrontSurface, upperHalf - 5, 9.5, 10, 8, COLORS.red, 0.8);
    model.rect(gableFrontSurface, upperHalf - 1, 9.5, 2, 8, COLORS.paper, 0);
    model.ellipseOn(gableFrontSurface, upperHalf - 1.8, 19, 1.8, 1.3, COLORS.paper, 0.6);
    model.ellipseOn(gableFrontSurface, upperHalf + 1.8, 19, 1.8, 1.3, COLORS.paper, 0.6);
  }
  const gableBackSurface = model.surface(roofGeometry.backSurface);
  model.ellipseOn(gableBackSurface, upperHalf, 14, 8, 8, COLORS.sky, 1);
  model.lineOn(gableBackSurface, [[upperHalf - 8, 14], [upperHalf + 8, 14]], 0.7);

  for (const slope of roofGeometry.slopes) {
    model.face(slope, COLORS.blue, { ink: 1.4 });
    if (theme.props.ornaments === "candy") model.shingles(model.surface(slope), 8, 12, INK);
    else seamedRoof(model, model.surface(slope), 12);
  }

  // A crooked chimney on one slope, and a balloon bunch tied to the other end of the ridge.
  model.group([-30, -22, ridgeHeight], 12);
  const chimney = model.box(-30, -22, eaveHeight + 14, 14, 14, 34, COLORS.red, { top: COLORS.plumDark, ink: 1.2 });
  for (const wall of [chimney.front, chimney.right, chimney.back, chimney.left]) {
    for (const brickV of [8, 15, 22]) model.lineOn(wall, [[2, brickV], [12, brickV + 0.6]], 0.55, INK, 0.45);
    model.rect(wall, 0, 28, wall.width, 3, COLORS.paper, 0.7);
  }
  const anchorZ = ridgeHeight - 1;
  model.group([36, 0, ridgeHeight + 20], 30);
  const balloons: [Vector3, string][] = [
    [[30, -11, anchorZ + 36], COLORS.pink],
    [[43, 9, anchorZ + 38], COLORS.yellow],
    [[34, 13, anchorZ + 26], COLORS.green],
    [[47, -8, anchorZ + 25], COLORS.purple],
    [[38, 0, anchorZ + 47], COLORS.red],
  ];
  for (const [position] of balloons) {
    const [anchorScreen, balloonScreen] = [model.toScreen([38, 0, anchorZ]), model.toScreen(position)];
    model.raw(`<path d="M${anchorScreen[0]} ${anchorScreen[1]}L${balloonScreen[0]} ${balloonScreen[1]}" stroke="${model.ink}" stroke-width="${model.lineWidth(0.7)}"/>`);
  }
  for (const [position, color] of balloons) model.ball(position, 7.2, color, { ink: 1.2, highlight: 0.7 });
  return model.render();
}
