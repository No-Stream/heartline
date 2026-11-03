import { INK } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model, storybookWarp, type Surface } from "./solid.ts";
import type { Theme } from "./theme.ts";

export function drawTicketBooth(theme: Theme, view: number): string {
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
