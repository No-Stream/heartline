// Buildings and stalls. Each is modelled once; all four views come from the same description.
// Front is +x, so rotation 0 shows the front on the right-hand wall and the +y side on the left-hand wall.
import { TILE_EDGE, type Vector3 } from "./iso.ts";
import { INK, shadePlum } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model, storybookWarp, type Surface } from "./solid.ts";
import type { Theme } from "./theme.ts";

export interface BuildingDesign {
  id: string;
  description: string;
  /** Tiles covered at rotation 0. */
  footprintX: number;
  footprintY: number;
  canvasWidth: number;
  canvasHeight: number;
  /** Height of the footprint centre from the top of the canvas. */
  anchorY: number;
  draw: (theme: Theme, view: number) => string;
}

function striped(model: Model, surface: Surface, stripeCount: number, firstColor: string, secondColor: string, ink = 0.8): void {
  const stripeWidth = surface.width / stripeCount;
  for (let index = 0; index < stripeCount; index++) {
    model.rect(surface, index * stripeWidth, 0, stripeWidth, surface.height, index % 2 ? secondColor : firstColor, ink);
  }
}

/** A sloped striped awning that sticks out from a wall, with a scalloped front edge. */
function awningOnFront(
  model: Model,
  frontX: number,
  centerY: number,
  awningWidth: number,
  wallHeight: number,
  reach: number,
  drop: number,
  stripeCount: number,
  firstColor: string,
  secondColor: string,
): void {
  const [y0, y1] = [centerY - awningWidth / 2, centerY + awningWidth / 2];
  const stripeWidth = awningWidth / stripeCount;
  const outerX = frontX + reach;
  const outerZ = wallHeight - drop;
  for (let index = 0; index < stripeCount; index++) {
    const [start, end] = [y0 + index * stripeWidth, y0 + (index + 1) * stripeWidth];
    const color = index % 2 ? secondColor : firstColor;
    model.face([[outerX, start, outerZ], [outerX, end, outerZ], [frontX, end, wallHeight], [frontX, start, wallHeight]], color, { ink: 0.9 });
    const hem: Vector3[] = [];
    const scallopDepth = model.theme.props.scallops ? 3 : 0;
    for (let step = 0; step <= 6; step++) {
      const fraction = step / 6;
      hem.push([outerX, start + (end - start) * (1 - fraction), outerZ - 2.2 - Math.sin(fraction * Math.PI) * scallopDepth]);
    }
    model.face([[outerX, start, outerZ], ...hem, [outerX, end, outerZ]].reverse() as Vector3[], shadePlum(color, 0.08), { ink: 0.9, lit: false, wobbleAmount: 0.05 });
  }
}

/** Thin vertical ribs across a band of a wall, as on a pressed metal or panelled skirt. */
function ribbed(model: Model, surface: Surface, bottom: number, height: number, ribCount: number): void {
  for (let rib = 1; rib < ribCount; rib++) {
    const u = (surface.width * rib) / ribCount;
    model.lineOn(surface, [[u, bottom + 1], [u, bottom + height - 1]], 0.55, INK, 0.4);
  }
}

/** Standing seams running from the eave up to the ridge of a flat-sheeted roof slope. */
function seamedRoof(model: Model, slope: Surface, seamCount: number): void {
  for (let seam = 1; seam < seamCount; seam++) {
    const u = (slope.width * seam) / seamCount;
    model.lineOn(slope, [[u, 0.5], [u, slope.height - 0.5]], 0.6, INK, 0.5);
  }
}

/** A square sign plate with a person pictogram on it, skirted or not. */
function pictogramSign(model: Model, surface: Surface, centerU: number, bottom: number, skirted: boolean, figureColor: string): void {
  model.rect(surface, centerU - 3.8, bottom, 7.6, 9.6, COLORS.paper, 0.7);
  model.ellipseOn(surface, centerU, bottom + 7.4, 1.2, 1.2, figureColor, 0);
  model.paint(surface, skirted
    ? [[centerU - 2.4, bottom + 1.4], [centerU + 2.4, bottom + 1.4], [centerU + 1, bottom + 5.8], [centerU - 1, bottom + 5.8]]
    : [[centerU - 1.2, bottom + 1.4], [centerU + 1.2, bottom + 1.4], [centerU + 1.6, bottom + 5.8], [centerU - 1.6, bottom + 5.8]], figureColor, 0);
}

function drawBurgerStall(theme: Theme, view: number): string {
  const height = 58;
  const warp = storybookWarp([1.8, -2.4], 0.05, 1.4, height + 26, theme.crookedness);
  const model = new Model("burger_stall", view, theme, warp);
  const half = 33;
  model.shadow([[-half - 2, -half - 2, 0], [half + 14, -half - 2, 0], [half + 14, half + 3, 0], [-half - 2, half + 3, 0]]);

  // Crates and a gas bottle stand behind the stall, on the side opposite the serving hatch.
  model.group([-half - 7, -19, 0], -2);
  const crate = model.box(-half - 7, -19, 0, 11, 13, 10, COLORS.wood, { ink: 1 });
  model.lineOn(crate.left, [[0, 5], [crate.left.width, 5]], 0.6);
  model.lineOn(crate.right, [[0, 5], [crate.right.width, 5]], 0.6);
  model.box(-half - 6.5, -18, 10, 8, 9, 7, shadePlum(COLORS.wood, 0.05), { ink: 1 });
  model.group([-half - 6, -4, 0], -2);
  model.lathe(-half - 6, -4, [[4.4, 0], [4.8, 3], [4.8, 10], [3.2, 13.5], [1.5, 14.5]], COLORS.mint);

  // Main body with planks and a candy skirt.
  model.group([0, 0, 18]);
  const walls = model.box(0, 0, 0, half * 2, half * 2, height, COLORS.cream, { top: shadePlum(COLORS.cream, 0.1), hatch: 7 });
  for (const wall of [walls.front, walls.right, walls.back, walls.left]) {
    for (let plank = 1; plank < 7; plank++) {
      const u = (wall.width * plank) / 7;
      model.lineOn(wall, [[u, 11], [u + 0.3, height - 1]], 0.55, INK, 0.3);
    }
    model.rect(wall, 0, 0, wall.width, 11, COLORS.red, 0);
    if (theme.props.ornaments === "candy") {
      for (let dot = 0; dot < 6; dot++) model.ellipseOn(wall, ((dot + 0.5) / 6) * wall.width, 5.5, 2, 2.1, COLORS.paper, 0);
    } else {
      ribbed(model, wall, 0, 11, 12);
    }
    model.lineOn(wall, [[0, 11], [wall.width, 11]], 1);
  }

  // Front: the serving hatch with a cook inside, a menu fascia and a counter.
  const hatchLeft = 9;
  const hatchWidth = walls.front.width - 18;
  const hatchBottom = 19;
  const hatchHeight = 26;
  model.rect(walls.front, hatchLeft, hatchBottom, hatchWidth, hatchHeight, COLORS.plumDark, 1.1);
  if (walls.front.visible) {
    // Interior: a warm back wall, the cook and a little grill.
    model.rect(walls.front, hatchLeft + 1.5, hatchBottom + 1, hatchWidth - 3, hatchHeight - 5, "#a8552e", 0);
    model.ellipseOn(walls.front, half - 4, hatchBottom + 9, 5.2, 5.6, "#c98f62", 0.9);
    model.rect(walls.front, half - 8.8, hatchBottom + 13, 9.6, 3.6, COLORS.paper, 0.8);
    model.ellipseOn(walls.front, half - 8, hatchBottom + 17.2, 2.8, 2.8, COLORS.paper, 0.8);
    model.ellipseOn(walls.front, half - 4, hatchBottom + 18, 3.4, 3.1, COLORS.paper, 0.8);
    model.ellipseOn(walls.front, half, hatchBottom + 17.2, 2.8, 2.8, COLORS.paper, 0.8);
    model.ellipseOn(walls.front, half - 5.8, hatchBottom + 9.5, 0.9, 1.1, INK, 0);
    model.ellipseOn(walls.front, half - 2, hatchBottom + 9.5, 0.9, 1.1, INK, 0);
    model.strokeOn(walls.front, [[half - 6, hatchBottom + 6.5], [half - 4, hatchBottom + 5.5], [half - 2, hatchBottom + 6.5]], 0.8);
    model.rect(walls.front, hatchLeft + 8, hatchBottom + 0.5, 8, 2.6, COLORS.yellow, 0.7);
  }

  // Counter shelf with ketchup and mustard bottles.
  model.group([half + 6, 0, 18], 4);
  const shelf = model.box(half + 4, 0, 16.5, 10, hatchWidth + 8, 2.6, COLORS.peach, { ink: 1.1 });
  model.lineOn(shelf.front, [[1, 1.3], [shelf.front.width - 1, 1.3]], 0.5, INK, 0.45);
  for (const [bottleY, bottleColor] of [[-12, COLORS.red], [-7.5, COLORS.yellow]] as const) {
    model.lathe(half + 5, bottleY, [[1.8, 19], [1.9, 24], [0.8, 26], [0.4, 28]], bottleColor, { ink: 0.9, shadeSide: false });
  }

  // Sign fascia, striped awning, then the roof slab.
  model.group([half + 1, 0, height], 5);
  awningOnFront(model, half + 0.5, 0, half * 2 - 2, height - 1, 11, 7, 7, COLORS.red, COLORS.paper);

  model.group([0, 0, height + 1], 3);
  const roof = model.box(0, 0, height, half * 2 + 8, half * 2 + 8, 11, COLORS.pink, { top: COLORS.yellow, ink: 1.3 });
  model.letter(roof.front, "BURGERS", roof.front.width / 2, 2, 7.4, COLORS.paper, 1.25);
  model.letter(roof.back, "YUM", roof.back.width / 2, 2, 7.4, COLORS.paper, 1.25);
  for (const wall of [roof.right, roof.left]) {
    if (theme.props.ornaments === "candy") {
      for (let dot = 0; dot < 8; dot++) model.ellipseOn(wall, ((dot + 0.5) / 8) * wall.width, 5.5, 2, 2, COLORS.paper, 0);
    } else {
      model.rect(wall, 0, 4.4, wall.width, 2.2, COLORS.paper, 0);
    }
  }

  // Right side: a menu board with a little hand-drawn burger.
  model.group([0, half + 1, 26], 2);
  model.rect(walls.right, 14, 19, 38, 27, COLORS.plumDark, 1.2);
  if (walls.right.visible) {
    model.ellipseOn(walls.right, 33, 37, 8, 4, COLORS.peach, 0.7);
    model.rect(walls.right, 25.5, 32, 15, 2, "#7a3d22", 0.5);
    model.rect(walls.right, 25, 34, 16, 1.4, COLORS.green, 0);
    model.rect(walls.right, 26, 29.6, 14, 2.4, COLORS.peach, 0.6);
    model.lineOn(walls.right, [[20, 25], [46, 25]], 0.8, COLORS.paper);
    model.lineOn(walls.right, [[24, 22.5], [42, 22.5]], 0.8, COLORS.yellow);
  }

  // Back: a staff door. Left: a round window and a string of hanging pans.
  model.arch(walls.back, 39, 0, 20, 40, COLORS.leafTeal, 1.1);
  model.ellipseOn(walls.back, 54.5, 17, 1.2, 1.2, COLORS.yellow, 0.6);
  model.ellipseOn(walls.back, 49, 30, 4, 4, COLORS.sky, 0.8);
  model.ellipseOn(walls.left, 33, 33, 10, 10, COLORS.yellow, 1.1);
  model.ellipseOn(walls.left, 33, 33, 7.2, 7.2, COLORS.sky, 0.9);
  model.lineOn(walls.left, [[33, 26], [33, 40]], 0.6);
  model.lineOn(walls.left, [[26, 33], [40, 33]], 0.6);

  // A vent pipe, then the giant burger that makes the stall read from a distance.
  const roofTop = height + 11;
  model.group([-26, -26, roofTop], 6);
  model.pipe(-26, -26, roofTop, roofTop + 11, 3.2, "#c8c2d6");

  model.group([0, 0, roofTop + 14], 12);
  model.lathe(0, 0, [[19, roofTop + 1], [20, roofTop + 3], [20, roofTop + 6], [18.5, roofTop + 7.5]], COLORS.peach);
  model.lathe(0, 0, [[21, roofTop + 7], [21, roofTop + 12]], "#7a3d22");
  const lettuceBase = roofTop + 12;
  const frills: Vector3[] = Array.from({ length: 14 }, (_, index) => {
    const angle = (index / 14) * Math.PI * 2;
    return [Math.cos(angle) * 23, Math.sin(angle) * 23, lettuceBase + (index % 2 ? 1.2 : -0.6)];
  });
  for (const frill of frills) {
    const viewPoint = model.toView(frill);
    if (viewPoint[0] + viewPoint[1] > -10) model.ball(frill, 4.2, COLORS.leafGreen, { ink: 1, highlight: 0 });
  }
  model.lathe(0, 0, [[21.5, lettuceBase - 1], [22.5, lettuceBase + 2.5], [20, lettuceBase + 3]], COLORS.yellow, { ink: 1, shadeSide: false });
  model.lathe(0, 0, [[20, lettuceBase + 3], [20.5, lettuceBase + 7], [18, lettuceBase + 13], [12, lettuceBase + 18], [5, lettuceBase + 20.5], [0.1, lettuceBase + 21]], COLORS.peach);
  const seeds: Vector3[] = [[3, -8, lettuceBase + 19], [11, 2, lettuceBase + 17.5], [-4, 7, lettuceBase + 19.5], [8, 10, lettuceBase + 15.5], [-11, -5, lettuceBase + 16.5], [13, -7, lettuceBase + 14.5], [0, 15, lettuceBase + 14], [-14, 8, lettuceBase + 13]];
  for (const seed of seeds) {
    const [screenX, screenY] = model.toScreen(seed);
    model.raw(`<ellipse cx="${screenX}" cy="${screenY}" rx="1.5" ry="0.95" fill="${model.color(COLORS.paper)}" stroke="${model.ink}" stroke-width="${model.lineWidth(0.4)}" transform="rotate(${(seed[0] * 13) % 40} ${screenX} ${screenY})"/>`);
  }
  return model.render();
}

function drawRestroom(theme: Theme, view: number): string {
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
  const gableFront: Vector3[] = [[half, -half, wallHeight], [half, half, wallHeight], [half, 0, ridgeHeight - 2]];
  const gableBack: Vector3[] = [[-half, half, wallHeight], [-half, -half, wallHeight], [-half, 0, ridgeHeight - 2]];
  model.face(gableFront, COLORS.paper, { ink: 1 });
  model.face(gableBack, COLORS.paper, { ink: 1 });
  const gableFrontSurface = model.surface([[half, -half, wallHeight], [half, half, wallHeight], [half, half, ridgeHeight], [half, -half, ridgeHeight]]);
  model.ellipseOn(gableFrontSurface, half, 11, 7.8, 7.8, COLORS.yellow, 1);
  model.letter(gableFrontSurface, "WC", half, 7.8, 6.2, INK, 1.1);
  const gableBackSurface = model.surface([[-half, half, wallHeight], [-half, -half, wallHeight], [-half, -half, ridgeHeight], [-half, half, ridgeHeight]]);
  model.ellipseOn(gableBackSurface, half, 10.5, 5, 5, COLORS.sky, 0.9);

  const slopeOne: [Vector3, Vector3, Vector3, Vector3] = [[edge, edge, wallHeight - 5], [-edge, edge, wallHeight - 5], [-edge, 0, ridgeHeight], [edge, 0, ridgeHeight]];
  const slopeTwo: [Vector3, Vector3, Vector3, Vector3] = [[-edge, -edge, wallHeight - 5], [edge, -edge, wallHeight - 5], [edge, 0, ridgeHeight], [-edge, 0, ridgeHeight]];
  for (const slope of [slopeOne, slopeTwo]) {
    model.face(slope, COLORS.violet, { ink: 1.3 });
    if (theme.props.ornaments === "candy") model.shingles(model.surface(slope), 6, 9, INK);
    else seamedRoof(model, model.surface(slope), 9);
  }

  // Vent pipe with a little cap on the back slope.
  model.group([-16, -14, ridgeHeight], 10);
  model.pipe(-16, -14, wallHeight + 16, ridgeHeight + 4, 3.2, COLORS.peach);
  return model.render();
}

function drawTicketBooth(theme: Theme, view: number): string {
  const wallHeight = 40;
  const warp = storybookWarp([2.2, 1.6], 0.03, 1, 100, theme.crookedness);
  const model = new Model("ticket_booth", view, theme, warp);
  const radius = 29;
  model.groundEllipse(radius + 4, radius + 4, 0.24, 2, 2);

  // Rope barrier posts stand ahead of the window and are drawn in depth order with the booth.
  for (const postY of [-24, 24]) {
    model.group([radius + 11, postY, 8], 1);
    model.lathe(radius + 11, postY, [[3, 0], [1.2, 1.5], [1.2, 14], [2, 15.5], [0.1, 17.5]], COLORS.yellow, { ink: 0.9, shadeSide: false });
  }
  model.group([radius + 11, 0, 12], 1);
  model.tube([[radius + 11, -24, 13], [radius + 11, -12, 9], [radius + 11, 0, 7.5], [radius + 11, 12, 9], [radius + 11, 24, 13]], 1.1, COLORS.red, 0.5);

  // A stack of tiny posters leans behind, on the side with the door.
  model.group([0, 0, wallHeight / 2]);
  model.lathe(0, 0, [[radius, 0], [radius, wallHeight]], COLORS.yellow, { ink: 1.3, shadeSide: true });
  const fullArc = (angle: number, arc: number): Surface => model.cylinderSurface(0, 0, 0, radius, wallHeight, angle, arc);
  for (let index = 0; index < 16; index++) {
    const angle = (index / 16) * Math.PI * 2;
    const stripe = fullArc(angle, 6);
    if (stripe.visible && index % 2 === 0) model.rect(stripe, 0, 0, 6, wallHeight, COLORS.paper, 0);
  }
  for (const angle of [0, Math.PI / 2, Math.PI, -Math.PI / 2, Math.PI / 4, -Math.PI / 4, (3 * Math.PI) / 4, (-3 * Math.PI) / 4]) {
    const skirt = fullArc(angle, 24);
    if (skirt.visible) model.rect(skirt, 0, 0, 24, 8, COLORS.violet, 0);
  }

  // Front: an arched ticket window with a cashier, a little ledge and a ticket tacked above.
  const windowPatch = fullArc(0, 34);
  model.arch(windowPatch, 5, 13, 24, 25, COLORS.plumDark, 1.3);
  if (windowPatch.visible) {
    model.arch(windowPatch, 7, 13.5, 20, 21, "#a8552e", 0);
    model.ellipseOn(windowPatch, 17, 25, 5.4, 5.6, "#8f5a3c", 0.9);
    model.ellipseOn(windowPatch, 15, 26, 0.9, 1.1, INK, 0);
    model.ellipseOn(windowPatch, 19, 26, 0.9, 1.1, INK, 0);
    model.strokeOn(windowPatch, [[15, 22.8], [17, 22], [19, 22.8]], 0.8);
    model.paint(windowPatch, [[11.5, 27.5], [22.5, 27.5], [20.5, 33], [13.5, 33]], COLORS.red, 0.8);
    model.rect(windowPatch, 8, 13.5, 18, 6.5, COLORS.green, 0.8);
  }
  model.group([radius + 5, 0, 13], 6);
  const ledgeSurface = model.box(radius + 2, 0, 12, 7, 26, 2.4, COLORS.peach, { ink: 1 });
  model.lineOn(ledgeSurface.front, [[1, 1.2], [25, 1.2]], 0.5, INK, 0.45);

  // Right and left: posters. Back: a little door with a star.
  model.group([0, 0, wallHeight / 2], 0.5);
  const posterRight = fullArc(Math.PI / 2, 24);
  model.rect(posterRight, 1, 12, 22, 23, COLORS.pink, 1.1);
  model.ellipseOn(posterRight, 12, 27, 5, 5, COLORS.yellow, 0.8);
  model.strokeOn(posterRight, [[5, 15.5], [8, 21], [12, 16.5], [16, 21], [19, 15.5]], 1.3, COLORS.paper);
  const backDoor = fullArc(Math.PI, 26);
  model.arch(backDoor, 3, 0, 20, 33, COLORS.leafTeal, 1.2);
  model.strokeOn(backDoor, [[13, 16], [14.6, 20.6], [19, 21], [15.4, 23.6], [16.8, 28], [13, 25.4], [9.2, 28], [10.6, 23.6], [7, 21], [11.4, 20.6], [13, 16]], 1, COLORS.yellow);
  const posterLeft = fullArc(-Math.PI / 2, 24);
  model.rect(posterLeft, 1, 12, 22, 23, COLORS.sky, 1.1);
  model.strokeOn(posterLeft, [[4, 16], [8, 29], [12, 18], [15, 25], [20, 16]], 1.5, COLORS.blue);
  model.ellipseOn(posterLeft, 17.5, 30, 2.6, 2.6, COLORS.yellow, 0.7);

  // Candy-striped cone with a pennant on the tip.
  const roofBottom = wallHeight - 2;
  // Without circus stripes, a lower roof reads as a kiosk rather than a tent.
  const apex = wallHeight + (theme.props.coneStripes ? 37 : 26);
  model.group([0, 0, wallHeight + 15], 10);
  model.stripedCone(0, 0, radius + 9, roofBottom, apex, [COLORS.red, COLORS.paper], 14);
  // Roof banner on the front of the cone, facing +x only, carries the word.
  const roofFront = model.surface([[radius + 7, -22, wallHeight + 1], [radius + 7, 22, wallHeight + 1], [radius - 2, 19, wallHeight + 15], [radius - 2, -19, wallHeight + 15]]);
  model.group([radius + 4, 0, wallHeight + 8], 16);
  if (roofFront.visible) {
    model.rect(roofFront, 0, 0, roofFront.width, roofFront.height, COLORS.paper, 1.1);
    model.letter(roofFront, "TICKETS", roofFront.width / 2, 2.6, 5.6, COLORS.red, 1.25);
  }
  const roofBackBanner = model.surface([[-radius - 7, 16, wallHeight + 1], [-radius - 7, -16, wallHeight + 1], [-radius + 2, -14, wallHeight + 13], [-radius + 2, 14, wallHeight + 13]]);
  if (roofBackBanner.visible) {
    model.rect(roofBackBanner, 0, 0, roofBackBanner.width, roofBackBanner.height, COLORS.paper, 1);
    model.letter(roofBackBanner, "FUN", roofBackBanner.width / 2, 1.8, 5.4, COLORS.violet, 1.2);
  }
  model.group([0, 0, apex + 8], 20);
  model.stick([0, 0, apex - 1], [0, 0, apex + 15], 1, COLORS.woodDark, 0.7);
  const pennant = model.surface([[0, 0, apex + 7], [0, 14, apex + 7], [0, 14, apex + 15], [0, 0, apex + 15]]);
  const pennantBack = model.surface([[0, 14, apex + 7], [0, 0, apex + 7], [0, 0, apex + 15], [0, 14, apex + 15]]);
  model.paint(pennant, [[14, 0], [0, 4], [14, 8]], COLORS.yellow, 0.9);
  model.paint(pennantBack, [[0, 0], [14, 4], [0, 8]], COLORS.yellow, 0.9);
  model.ball([0, 0, apex], 2.6, COLORS.yellow, { ink: 1, highlight: 0.7 });
  return model.render();
}

function drawSouvenirShop(theme: Theme, view: number): string {
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
  const gableFront: Vector3[] = [[upperHalf, -upperHalf, eaveHeight], [upperHalf, upperHalf, eaveHeight], [upperHalf, 0, ridgeHeight - 3]];
  const gableBack: Vector3[] = [[-upperHalf, upperHalf, eaveHeight], [-upperHalf, -upperHalf, eaveHeight], [-upperHalf, 0, ridgeHeight - 3]];
  model.face(gableFront, COLORS.pink, { ink: 1.1 });
  model.face(gableBack, COLORS.pink, { ink: 1.1 });
  const gableFrontSurface = model.surface([[upperHalf, -upperHalf, eaveHeight], [upperHalf, upperHalf, eaveHeight], [upperHalf, upperHalf, ridgeHeight], [upperHalf, -upperHalf, ridgeHeight]]);
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
  const gableBackSurface = model.surface([[-upperHalf, upperHalf, eaveHeight], [-upperHalf, -upperHalf, eaveHeight], [-upperHalf, -upperHalf, ridgeHeight], [-upperHalf, upperHalf, ridgeHeight]]);
  model.ellipseOn(gableBackSurface, upperHalf, 14, 8, 8, COLORS.sky, 1);
  model.lineOn(gableBackSurface, [[upperHalf - 8, 14], [upperHalf + 8, 14]], 0.7);

  const slopeOne: [Vector3, Vector3, Vector3, Vector3] = [[edge, edge, eaveHeight - 6], [-edge, edge, eaveHeight - 6], [-edge, 0, ridgeHeight], [edge, 0, ridgeHeight]];
  const slopeTwo: [Vector3, Vector3, Vector3, Vector3] = [[-edge, -edge, eaveHeight - 6], [edge, -edge, eaveHeight - 6], [edge, 0, ridgeHeight], [-edge, 0, ridgeHeight]];
  for (const slope of [slopeOne, slopeTwo]) {
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

function drawParkGate(theme: Theme, view: number): string {
  const towerHeight = 72;
  const warp = storybookWarp([0, 0], 0.03, 1, 120, theme.crookedness);
  const model = new Model("park_gate", view, theme, warp);
  const towerY = TILE_EDGE * 1.65;
  const radius = 16;
  for (const side of [-1, 1]) {
    const y = side * towerY;
    model.groundEllipse(radius + 3, radius + 3, 0.24, 2, y + 2);
    model.group([0, y, towerHeight / 2]);
    model.lathe(0, y, [[radius + 2, 0], [radius + 2, 7], [radius, 8.5], [radius, towerHeight]], COLORS.pink, { ink: 1.3 });
    model.lathe(0, y, [[radius + 2, 0], [radius + 2, 7], [radius, 8.5]], COLORS.purple, { ink: 1 });
    for (const [windowAngle, tint] of [[0, COLORS.yellow], [Math.PI, COLORS.yellow], [Math.PI / 2, COLORS.sky], [-Math.PI / 2, COLORS.sky]] as const) {
      const patch = model.cylinderSurface(0, y, 0, radius, towerHeight, windowAngle, 18);
      model.arch(patch, 3.5, 21, 11, 21, tint, 1.1);
      model.lineOn(patch, [[9, 21], [9, 41]], 0.6);
    }
    model.group([0, y, towerHeight + 18], 10);
    model.stripedCone(0, y, radius + 6, towerHeight - 2, towerHeight + 34, [COLORS.yellow, COLORS.violet], 10);
    model.group([0, y, towerHeight + 40], 16);
    model.stick([0, y, towerHeight + 33], [0, y, towerHeight + 49], 1, COLORS.woodDark, 0.7);
    model.ball([0, y, towerHeight + 33], 2.5, COLORS.red, { ink: 1, highlight: 0.7 });
    const flag = model.surface([[0, y, towerHeight + 41], [0, y + 12, towerHeight + 41], [0, y + 12, towerHeight + 49], [0, y, towerHeight + 49]]);
    const flagBack = model.surface([[0, y + 12, towerHeight + 41], [0, y, towerHeight + 41], [0, y, towerHeight + 49], [0, y + 12, towerHeight + 49]]);
    model.paint(flag, [[12, 0], [0, 4], [12, 8]], COLORS.red, 0.9);
    model.paint(flagBack, [[0, 0], [12, 4], [0, 8]], COLORS.red, 0.9);
  }

  // The arched banner runs between the towers, with a different message on each face.
  const bannerBottom = 52;
  const bannerHeight = 15;
  const segments = 10;
  const span = towerY - 8;
  const archHeight = (y: number): number => 11 * Math.cos(((y / span) * Math.PI) / 2);
  model.group([0, 0, bannerBottom + 5], 2);
  for (const [faceX, direction] of [[3, 1], [-3, -1]] as const) {
    for (let index = 0; index < segments; index++) {
      const y0 = -span + (index / segments) * span * 2;
      const y1 = -span + ((index + 1) / segments) * span * 2;
      const [yStart, yEnd] = direction === 1 ? [y0, y1] : [y1, y0];
      const corners: Vector3[] = [
        [faceX, yStart, bannerBottom + archHeight(yStart)],
        [faceX, yEnd, bannerBottom + archHeight(yEnd)],
        [faceX, yEnd, bannerBottom + bannerHeight + archHeight(yEnd)],
        [faceX, yStart, bannerBottom + bannerHeight + archHeight(yStart)],
      ];
      model.face(corners, index % 2 ? COLORS.yellow : "#ffd653", { ink: 0, wobbleAmount: 0.05 });
    }
    const banner = model.surface([
      [faceX, direction * -span, bannerBottom],
      [faceX, direction * span, bannerBottom],
      [faceX, direction * span, bannerBottom + bannerHeight],
      [faceX, direction * -span, bannerBottom + bannerHeight],
    ]);
    if (banner.visible) {
      const edgeLine = (offset: number): [number, number][] =>
        Array.from({ length: 13 }, (_, index) => {
          const u = (index / 12) * banner.width;
          const y = span - (index / 12) * span * 2;
          return [u, offset + archHeight(y)];
        });
      model.strokeOn(banner, edgeLine(0), 1.2);
      model.strokeOn(banner, edgeLine(bannerHeight), 1.2);
      const message = direction === 1 ? "WELCOME" : "COME BACK";
      const letterSize = 8.4;
      const spacing = letterSize * 0.87;
      [...message].forEach((character, index) => {
        const u = banner.width / 2 + (index - (message.length - 1) / 2) * spacing;
        const y = span - (u / banner.width) * span * 2;
        model.letter(banner, character, u, 3.2 + archHeight(y), letterSize, COLORS.violet, 1.45);
      });
    }
  }
  return model.render();
}

export const BUILDINGS: BuildingDesign[] = [
  { id: "burger_stall", description: "Burger stall with a giant burger on the roof, a cook in the hatch, a menu board and a staff door", footprintX: 2, footprintY: 2, canvasWidth: 148, canvasHeight: 176, anchorY: 134, draw: drawBurgerStall },
  { id: "restroom", description: "Restroom cottage with two doors, a shingled roof, a water tank at the back and a flower box", footprintX: 2, footprintY: 2, canvasWidth: 148, canvasHeight: 152, anchorY: 110, draw: drawRestroom },
  { id: "ticket_booth", description: "Round ticket booth with a circus cone roof, a cashier, posters and rope posts", footprintX: 2, footprintY: 2, canvasWidth: 148, canvasHeight: 158, anchorY: 116, draw: drawTicketBooth },
  { id: "souvenir_shop", description: "Crooked half-timbered gift shop with a teddy in the window and balloons tied to the roof", footprintX: 3, footprintY: 3, canvasWidth: 208, canvasHeight: 256, anchorY: 196, draw: drawSouvenirShop },
  { id: "park_gate", description: "Park entrance: two towers joined by an arched banner reading WELCOME and, from behind, COME BACK", footprintX: 1, footprintY: 4, canvasWidth: 196, canvasHeight: 218, anchorY: 164, draw: drawParkGate },
];
