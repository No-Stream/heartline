import { type Vector3 } from "./iso.ts";
import { INK, shadePlum } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model, storybookWarp } from "./solid.ts";
import type { Theme } from "./theme.ts";
import { awningOnFront, ribbed } from "./building-primitives.ts";

export function drawBurgerStall(theme: Theme, view: number): string {
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
