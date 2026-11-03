import { TILE_EDGE, type Vector3 } from "./iso.ts";
import { COLORS } from "./palette.ts";
import { Model, storybookWarp } from "./solid.ts";
import type { Theme } from "./theme.ts";

export function drawParkGate(theme: Theme, view: number): string {
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
