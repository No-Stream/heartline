// Scenery items. Round things that look the same from all sides use the single view "any".
import { TILE_EDGE, type Vector3 } from "./iso.ts";
import { INK, type Point, shadePlum, smoothPath, wobble } from "./ink.ts";
import { COLORS } from "./palette.ts";
import { Model } from "./solid.ts";

export interface SceneryDesign {
  id: string;
  description: string;
  width: number;
  height: number;
  anchorY: number;
  rotates: boolean;
  frames: number;
  draw: (view: number, frame: number) => string;
}

function drawLollipopTree(
  id: string,
  crown: string,
  curl: string,
  crownRadius: number,
  trunkHeight: number,
  direction: number,
): string {
  const model = new Model(id, 0);
  model.groundEllipse(10, 10, 0.26, 3, 3);
  const lean = direction * 3;
  model.tube([[0, 0, 0], [lean * 0.2, -lean * 0.2, trunkHeight * 0.4], [lean, -lean, trunkHeight * 0.75], [lean * 0.6, -lean * 0.6, trunkHeight]], 3.2, COLORS.trunk, 1.1);
  const [crownX, crownY] = model.toScreen([lean * 0.6, -lean * 0.6, trunkHeight + crownRadius * 0.9]);
  model.raw(model.blob(crownX, crownY, crownRadius, crown, 1.4));
  // A lighter rim on the sunlit side and a darker one on the other give the flat disc a little volume.
  model.spiral(crownX - 0.5, crownY + 0.5, crownRadius * 0.84, curl, 2.75, crownRadius * 0.115, direction);
  model.raw(`<ellipse cx="${crownX - crownRadius * 0.45}" cy="${crownY - crownRadius * 0.52}" rx="${crownRadius * 0.15}" ry="${crownRadius * 0.1}" fill="#ffffff" fill-opacity="0.6" transform="rotate(-38 ${crownX - crownRadius * 0.45} ${crownY - crownRadius * 0.52})"/>`);
  return model.render();
}

function drawCloudTree(id: string, color: string, accent: string): string {
  const model = new Model(id, 0);
  model.groundEllipse(14, 14, 0.26, 3, 3);
  model.tube([[0, 0, 0], [2, -1, 12], [-1, 2, 24], [1, 0, 34]], 4, COLORS.trunk, 1.1);
  model.tube([[-1, 2, 22], [-6, 8, 28]], 2.2, COLORS.trunk, 0.9);
  model.tube([[1, -1, 18], [7, -7, 26]], 2.2, COLORS.trunk, 0.9);
  const puffs: [number, number, number][] = [[0, 42, 13], [-13, 34, 10], [13, 35, 10.5], [-8, 50, 9.5], [9, 49, 10]];
  const origin = model.toScreen([0, 0, 0]);
  for (const [screenOffset, up, radius] of puffs) {
    model.raw(model.blob(origin[0] + screenOffset, origin[1] - up, radius + 0.7, INK, 1.6));
  }
  for (const [screenOffset, up, radius] of puffs) {
    model.raw(model.blob(origin[0] + screenOffset, origin[1] - up, radius, color, 0));
  }
  for (const [screenOffset, up, radius] of puffs) {
    model.spiral(origin[0] + screenOffset, origin[1] - up, radius * 0.6, accent, 1.6, 0.95, screenOffset > 0 ? 1 : -1);
  }
  return model.render();
}

function drawPine(id: string, color: string): string {
  const model = new Model(id, 0);
  model.groundEllipse(10, 10, 0.26, 3, 3);
  model.tube([[0, 0, 0], [0, 0, 11]], 3.6, COLORS.trunk, 1.1);
  const origin = model.toScreen([0, 0, 0]);
  const tiers: [number, number, number][] = [[14, 9, 34], [24, 7.5, 28], [36, 6, 21]];
  for (const [base, halfHeightScale, height] of tiers) {
    const halfWidth = halfHeightScale * 1.65;
    const points: Point[] = [[origin[0] - halfWidth, origin[1] - base], [origin[0] - halfWidth * 0.45, origin[1] - base + 3], [origin[0] + halfWidth * 0.2, origin[1] - base + 3.6], [origin[0] + halfWidth, origin[1] - base], [origin[0] + 1, origin[1] - base - height + 8], [origin[0] - 1, origin[1] - base - height + 8]];
    const path = smoothPath(wobble(points, 0.3, model.randomSource), true, 0.6);
    model.raw(`<path d="${path}" fill="${color}" stroke="${INK}" stroke-width="1.2" stroke-linejoin="round"/>`);
    model.spiral(origin[0] + 2.5, origin[1] - base - 6, 3, INK, 1.2, 0.7);
  }
  model.ball([0, 0, 66], 2.8, COLORS.yellow, { ink: 1, highlight: 0.8 });
  return model.render();
}

function drawBush(id: string, color: string, flower: string | undefined): string {
  const model = new Model(id, 0);
  model.groundEllipse(15, 13, 0.24, 2, 2);
  const origin = model.toScreen([0, 0, 0]);
  const puffs: [number, number, number][] = [[-9, 7, 7.5], [8, 7.5, 8], [0, 12, 8.5], [-1, 4.5, 8]];
  for (const [offset, up, radius] of puffs) model.raw(model.blob(origin[0] + offset, origin[1] - up, radius + 0.65, INK, 1.4));
  for (const [offset, up, radius] of puffs) model.raw(model.blob(origin[0] + offset, origin[1] - up, radius, color, 0));
  model.spiral(origin[0] - 6, origin[1] - 8, 3.8, shadePlum(color, 0.55), 1.5, 0.75, -1);
  model.spiral(origin[0] + 6, origin[1] - 10, 3.8, shadePlum(color, 0.55), 1.5, 0.75, 1);
  if (flower) {
    for (const [offset, up] of [[-11, 11], [-3, 18], [5, 15], [12, 9], [1, 5], [-7, 3]]) {
      for (let petal = 0; petal < 5; petal++) {
        const angle = (petal / 5) * Math.PI * 2;
        model.raw(`<circle cx="${origin[0] + offset + Math.cos(angle) * 1.4}" cy="${origin[1] - up + Math.sin(angle) * 1.4}" r="1.05" fill="${flower}" stroke="${INK}" stroke-width="0.35"/>`);
      }
      model.raw(`<circle cx="${origin[0] + offset}" cy="${origin[1] - up}" r="0.75" fill="${COLORS.yellow}"/>`);
    }
  }
  return model.render();
}

function drawBench(view: number): string {
  const model = new Model("bench", view);
  const length = 34;
  model.shadow([[-9, -length / 2 - 1, 0], [11, -length / 2 - 1, 0], [11, length / 2 + 1, 0], [-9, length / 2 + 1, 0]], 0.22);
  // Backrest at -x, so the sitter looks towards +x.
  const legs: [number, number][] = [[-5, -length / 2 + 3], [-5, length / 2 - 3], [6, -length / 2 + 3], [6, length / 2 - 3]];
  for (const [legX, legY] of legs) {
    model.group([legX, legY, 3]);
    model.tube([[legX, legY, 0], [legX, legY, 9]], 1.8, COLORS.plumDark, 0.5);
  }
  model.group([0, 0, 10], 2);
  const seat = model.box(0.5, 0, 9, 14, length, 2.2, COLORS.red, { ink: 1.1 });
  model.lineOn(seat.front, [[1, 1.1], [length - 1, 1.1]], 0.4, INK, 0.4);
  model.group([-6.5, 0, 16], -0.5);
  const rest = model.box(-6, 0, 12, 2, length, 9.5, COLORS.red, { ink: 1.1 });
  for (const wall of [rest.front, rest.back]) {
    for (const slatV of [3.2, 6.4]) model.lineOn(wall, [[0.5, slatV], [wall.width - 0.5, slatV]], 0.7, INK, 0.75);
  }
  // Curled ends make it read as a park bench, not a box.
  for (const endY of [-length / 2 - 0.2, length / 2 + 0.2]) {
    model.group([-2, endY, 12], 3);
    model.tube([[-6, endY, 22.5], [-7.5, endY, 20], [-5, endY, 12], [3, endY, 11.8], [7.5, endY, 12], [8, endY, 15], [6, endY, 16]], 1.5, COLORS.yellow, 0.65);
  }
  return model.render();
}

function drawLamp(view: number): string {
  const model = new Model("lamp", view);
  model.groundEllipse(5, 5, 0.26, 1.6, 1.6);
  model.lathe(0, 0, [[4.2, 0], [4, 2.2], [1.6, 4.2]], COLORS.plumDark, { ink: 1, shadeSide: false });
  // The curly arm points along +x, so it swings around when the lamp rotates.
  model.tube([[0, 0, 3], [0, 0, 40], [1, 0, 48], [7, 0, 50], [13, 0, 46]], 2.1, COLORS.violet, 0.8);
  model.tube([[0, 0, 37], [-2.5, 0, 38.5], [-3.5, 0, 41.5], [-1.5, 0, 43]], 1.1, COLORS.violet, 0.6);
  const [bulbX, bulbY] = model.toScreen([13, 0, 40.5]);
  model.raw(`<circle cx="${bulbX}" cy="${bulbY}" r="8" fill="#fff2c2" fill-opacity="0.35"/>`);
  model.lathe(13, 0, [[5.6, 44.6], [3.4, 47.6], [0.1, 48.6]], COLORS.pink, { ink: 1, shadeSide: false });
  model.ball([13, 0, 41], 3.5, "#fff2a8", { ink: 1, highlight: 0.8 });
  return model.render();
}

function drawFlowerBed(view: number): string {
  const model = new Model("flower_bed", view);
  const sizeX = 40;
  const sizeY = 26;
  model.shadow([[-sizeX / 2 - 1, -sizeY / 2 - 1, 0], [sizeX / 2 + 3, -sizeY / 2 - 1, 0], [sizeX / 2 + 3, sizeY / 2 + 3, 0], [-sizeX / 2 - 1, sizeY / 2 + 3, 0]], 0.2);
  model.group([0, 0, 0]);
  const border = model.box(0, 0, 0, sizeX, sizeY, 5, COLORS.wood, { top: "#6d4632", ink: 1.1 });
  for (const wall of [border.front, border.right, border.back, border.left]) {
    for (let plank = 1; plank < 5; plank++) {
      const u = (wall.width * plank) / 5;
      model.lineOn(wall, [[u, 0.4], [u, 4.6]], 0.45, INK, 0.5);
    }
  }
  const bloomColors = [COLORS.pink, COLORS.yellow, COLORS.red, COLORS.purple, COLORS.paper, COLORS.orange];
  const flowers: { position: Vector3; color: string; height: number }[] = [];
  for (let row = 0; row < 4; row++) {
    for (let column = 0; column < 6; column++) {
      flowers.push({
        position: [-sizeX / 2 + 4.4 + column * 6.2 + (row % 2) * 1.8, -sizeY / 2 + 4 + row * 6, 5],
        color: bloomColors[(row * 5 + column * 2 + (column * row) % 3) % bloomColors.length],
        height: 6.5 + ((row * 7 + column * 3) % 5),
      });
    }
  }
  for (const { position, color, height } of flowers) {
    const [x, y, z] = position;
    model.group([x, y, z + height], 1);
    model.stick([x, y, z], [x, y, z + height], 0.8, COLORS.leafGreen, 0.35);
    const [flowerX, flowerY] = model.toScreen([x, y, z + height]);
    for (let petal = 0; petal < 5; petal++) {
      const angle = (petal / 5) * Math.PI * 2;
      model.raw(`<circle cx="${flowerX + Math.cos(angle) * 1.6}" cy="${flowerY + Math.sin(angle) * 1.5}" r="1.3" fill="${color}" stroke="${INK}" stroke-width="0.45"/>`);
    }
    model.raw(`<circle cx="${flowerX}" cy="${flowerY}" r="0.9" fill="${color === COLORS.yellow ? COLORS.orange : COLORS.yellow}"/>`);
  }
  return model.render();
}

function drawFence(view: number): string {
  const model = new Model("fence", view);
  // A picket section along one tile edge, at +x.
  const edgeX = TILE_EDGE / 2 - 1;
  const span = TILE_EDGE / 2;
  const picketCount = 7;
  model.shadow([[edgeX - 1.5, -span, 0], [edgeX + 1.5, -span, 0], [edgeX + 1.5, span, 0], [edgeX - 1.5, span, 0]], 0.18);
  model.group([edgeX, 0, 7]);
  for (const railZ of [5, 11]) {
    model.box(edgeX, 0, railZ, 1.2, span * 2, 1.6, COLORS.paper, { ink: 0.8, wobbleAmount: 0.15 });
  }
  for (let index = 0; index < picketCount; index++) {
    const y = -span + 3.2 + (index * (span * 2 - 6.4)) / (picketCount - 1);
    const pickets = model.toScreen([edgeX + 1, y, 0]);
    const tip = model.toScreen([edgeX + 1, y, 16 + (index % 2) * 1.2]);
    model.group([edgeX + 1, y, 8], 1);
    const halfWidth = 1.7;
    const shape: Point[] = [[pickets[0] - halfWidth, pickets[1]], [pickets[0] + halfWidth, pickets[1]], [tip[0] + halfWidth, tip[1] + 2.4], [tip[0], tip[1]], [tip[0] - halfWidth, tip[1] + 2.4]];
    model.raw(`<path d="M${shape.map(([x, y2]) => `${x} ${y2}`).join("L")}Z" fill="${index % 3 === 1 ? COLORS.pink : COLORS.paper}" stroke="${INK}" stroke-width="0.9" stroke-linejoin="round"/>`);
  }
  return model.render();
}

function drawBin(): string {
  const model = new Model("bin", 0);
  model.groundEllipse(7, 7, 0.26, 1.6, 1.6);
  model.lathe(0, 0, [[6.2, 0], [7, 16], [7, 17]], COLORS.green);
  const origin = model.toScreen([0, 0, 0]);
  for (let index = -2; index <= 2; index++) {
    const baseX = origin[0] + index * 2.6;
    model.raw(`<path d="M${baseX} ${origin[1] + 2}L${baseX * 1.05} ${origin[1] - 13}" stroke="${INK}" stroke-opacity="0.35" stroke-width="0.6"/>`);
  }
  model.lathe(0, 0, [[8, 16.4], [8, 18.2], [4.8, 20.4]], COLORS.yellow, { ink: 1.1, shadeSide: false });
  const [knobX, knobY] = model.toScreen([0, 0, 22]);
  model.raw(`<circle cx="${knobX}" cy="${knobY}" r="1.6" fill="${COLORS.red}" stroke="${INK}" stroke-width="0.8"/>`);
  // Everyone likes a smiling bin.
  const [faceX, faceY] = [origin[0], origin[1] - 8];
  model.raw(`<circle cx="${faceX - 2.6}" cy="${faceY - 1.5}" r="0.95" fill="${INK}"/><circle cx="${faceX + 2.6}" cy="${faceY - 1.5}" r="0.95" fill="${INK}"/><path d="M${faceX - 2.4} ${faceY + 1}Q${faceX} ${faceY + 3.6} ${faceX + 2.4} ${faceY + 1}" fill="none" stroke="${INK}" stroke-width="0.8" stroke-linecap="round"/>`);
  return model.render();
}

function drawFountain(_view: number, frame: number): string {
  const model = new Model("fountain", 0);
  model.groundEllipse(36, 36, 0.24, 2, 2);
  const basinRadius = 34;
  model.lathe(0, 0, [[basinRadius, 0], [basinRadius + 1, 8]], COLORS.purple, { ink: 1.4 });
  const rimPoints = Array.from({ length: 24 }, (_, index) => {
    const angle = (index / 24) * Math.PI * 2;
    return model.toScreen([Math.cos(angle) * (basinRadius + 1), Math.sin(angle) * (basinRadius + 1), 8]);
  });
  const rimInner = Array.from({ length: 24 }, (_, index) => {
    const angle = (index / 24) * Math.PI * 2;
    return model.toScreen([Math.cos(angle) * (basinRadius - 3), Math.sin(angle) * (basinRadius - 3), 8]);
  });
  model.raw(`<path d="${smoothPath(rimPoints, true)}" fill="${COLORS.pink}" stroke="${INK}" stroke-width="1.2"/>`);
  model.raw(`<path d="${smoothPath(rimInner, true)}" fill="${COLORS.water}" stroke="${INK}" stroke-width="1"/>`);
  // Rings spreading out across the water move with the frame.
  for (let ring = 0; ring < 2; ring++) {
    const growth = ((frame / 4 + ring / 2) % 1) * 0.8 + 0.25;
    const points = Array.from({ length: 16 }, (_, index) => {
      const angle = (index / 16) * Math.PI * 2;
      return model.toScreen([Math.cos(angle) * (basinRadius - 3) * growth, Math.sin(angle) * (basinRadius - 3) * growth, 8]);
    });
    model.raw(`<path d="${smoothPath(points, true)}" fill="none" stroke="#ffffff" stroke-opacity="${(1.05 - growth) * 0.9}" stroke-width="1"/>`);
  }
  model.lathe(0, 0, [[9, 8], [5, 12], [3.2, 22]], COLORS.mint, { ink: 1.2 });
  model.lathe(0, 0, [[15, 22], [16, 26], [3, 27]], COLORS.pink, { ink: 1.2 });
  const upperWater = Array.from({ length: 16 }, (_, index) => {
    const angle = (index / 16) * Math.PI * 2;
    return model.toScreen([Math.cos(angle) * 12.6, Math.sin(angle) * 12.6, 26.4]);
  });
  model.raw(`<path d="${smoothPath(upperWater, true)}" fill="${COLORS.water}" stroke="${INK}" stroke-width="0.8"/>`);
  model.lathe(0, 0, [[2.4, 26], [2.4, 34]], COLORS.mint, { ink: 1 });
  model.ball([0, 0, 37], 3.5, COLORS.yellow, { ink: 1, highlight: 0.8 });
  // Water arcs in candy blue, phased so that the droplets run outwards.
  const origin = model.toScreen([0, 0, 0]);
  for (let arc = 0; arc < 6; arc++) {
    const angle = (arc / 6) * Math.PI * 2 + 0.4;
    const reachX = Math.cos(angle) * 19;
    const reachY = Math.sin(angle) * 9.5;
    const startY = origin[1] - 35;
    const path = `M${origin[0]} ${startY}Q${origin[0] + reachX * 0.5} ${startY - 12} ${origin[0] + reachX} ${origin[1] - 14 + reachY}`;
    model.raw(`<path d="${path}" fill="none" stroke="${INK}" stroke-width="2.3" stroke-linecap="round"/>`);
    model.raw(`<path d="${path}" fill="none" stroke="${COLORS.sky}" stroke-width="1.2" stroke-linecap="round"/>`);
    const fraction = ((frame / 4 + arc / 6) % 1) * 0.92;
    const dropletX = (1 - fraction) ** 2 * origin[0] + 2 * (1 - fraction) * fraction * (origin[0] + reachX * 0.5) + fraction ** 2 * (origin[0] + reachX);
    const dropletY = (1 - fraction) ** 2 * startY + 2 * (1 - fraction) * fraction * (startY - 12) + fraction ** 2 * (origin[1] - 14 + reachY);
    model.raw(`<circle cx="${dropletX}" cy="${dropletY}" r="1.4" fill="#ffffff" stroke="${INK}" stroke-width="0.5"/>`);
  }
  return model.render();
}

function drawSignpost(view: number): string {
  const model = new Model("signpost", view);
  model.groundEllipse(5, 5, 0.24, 1.6, 1.6);
  model.group([0, 0, 0]);
  model.tube([[0, 0, 0], [0, 0, 34]], 2.6, COLORS.wood, 0.9);
  model.group([0, 0, 28], 2);
  // Two arrow boards at right angles, each lettered on both of its faces.
  const arrows: { direction: Vector3; word: string; color: string; height: number }[] = [
    { direction: [1, 0, 0], word: "RIDE", color: COLORS.red, height: 25 },
    { direction: [0, 1, 0], word: "FOOD", color: COLORS.yellow, height: 15 },
  ];
  for (const { direction, word, color, height } of arrows) {
    const tipX = direction[0] * 20;
    const tipY = direction[1] * 20;
    const startX = -direction[0] * 3;
    const startY = -direction[1] * 3;
    const shaftX = direction[0] * 15;
    const shaftY = direction[1] * 15;
    const sideOne = model.surface([[startX, startY, height], [shaftX, shaftY, height], [shaftX, shaftY, height + 8], [startX, startY, height + 8]]);
    const sideTwo = model.surface([[shaftX, shaftY, height], [startX, startY, height], [startX, startY, height + 8], [shaftX, shaftY, height + 8]]);
    for (const [surface, tipAtStart] of [[sideOne, true], [sideTwo, false]] as const) {
      if (!surface.visible) continue;
      const width = surface.width;
      const points: [number, number][] = tipAtStart
        ? [[width, 0], [width, 8], [0, 8], [-4.5, 4], [0, 0]]
        : [[0, 0], [width, 0], [width + 4.5, 4], [width, 8], [0, 8]];
      model.paint(surface, points, color, 0.95);
      model.letter(surface, word, width / 2, 1.8, 4.4, INK, 0.8);
    }
    void tipX;
    void tipY;
  }
  return model.render();
}

function drawMushroomStool(): string {
  const model = new Model("mushroom_stool", 0);
  model.groundEllipse(10, 10, 0.24, 2, 2);
  model.lathe(0, 0, [[5.8, 0], [4.2, 5], [3.8, 9]], COLORS.paper, { ink: 1.1 });
  model.lathe(0, 0, [[11, 8], [11, 10], [7, 15], [0.1, 17]], COLORS.red, { ink: 1.2 });
  const origin = model.toScreen([0, 0, 0]);
  for (const [offsetX, offsetY, radius] of [[-4, -14.6, 1.8], [2.6, -16.4, 1.3], [5, -12.2, 1.5], [-0.6, -11.4, 1.1]]) {
    model.raw(`<ellipse cx="${origin[0] + offsetX}" cy="${origin[1] + offsetY}" rx="${radius}" ry="${radius * 0.8}" fill="${COLORS.paper}"/>`);
  }
  return model.render();
}

function drawBalloonCart(view: number): string {
  const model = new Model("balloon_cart", view);
  model.shadow([[-14, -9, 0], [16, -9, 0], [16, 11, 0], [-14, 11, 0]], 0.22);
  // Wheels turn with the cart. Handle at -x.
  for (const wheelY of [-9.5, 9.5]) {
    model.group([0, wheelY, 5], wheelY > 0 ? 0 : -1);
    const wheelSurface = model.surface(
      wheelY > 0
        ? [[8, wheelY, 0], [-8, wheelY, 0], [-8, wheelY, 12], [8, wheelY, 12]]
        : [[-8, wheelY, 0], [8, wheelY, 0], [8, wheelY, 12], [-8, wheelY, 12]],
    );
    model.ellipseOn(wheelSurface, 8, 6, 6, 6, COLORS.yellow, 1.1);
    model.ellipseOn(wheelSurface, 8, 6, 1.6, 1.6, COLORS.red, 0.7);
  }
  model.group([0, 0, 10], 2);
  const cart = model.box(0, 0, 6, 24, 18, 13, COLORS.blue, { top: COLORS.paper, ink: 1.2 });
  for (const wall of [cart.front, cart.back, cart.right, cart.left]) {
    for (let star = 0; star < 3; star++) {
      model.ellipseOn(wall, ((star + 0.5) / 3) * wall.width, 6.5, 1.6, 1.6, COLORS.yellow, 0);
    }
  }
  model.group([-15, 0, 15], 1);
  model.tube([[-11, -6, 14], [-20, -6, 18], [-20, 6, 18], [-11, 6, 14]], 1.3, COLORS.plumDark, 0.5);
  model.group([0, 0, 30], 6);
  const balloons: [Vector3, string][] = [
    [[-5, -3, 46], COLORS.pink],
    [[6, 4, 49], COLORS.yellow],
    [[-1, 6, 58], COLORS.green],
    [[6, -6, 59], COLORS.red],
    [[-7, 3, 62], COLORS.purple],
    [[1, -1, 68], COLORS.blue],
  ];
  const anchor = model.toScreen([0, 0, 19]);
  for (const [position] of balloons) {
    const balloonScreen = model.toScreen(position);
    model.raw(`<path d="M${anchor[0]} ${anchor[1]}L${balloonScreen[0]} ${balloonScreen[1]}" stroke="${INK}" stroke-width="0.6"/>`);
  }
  for (const [position, color] of balloons) model.ball(position, 6, color, { ink: 1.1, highlight: 0.7 });
  return model.render();
}

const tree = (id: string, description: string, draw: () => string, height = 112): SceneryDesign => ({
  id,
  description,
  width: 64,
  height,
  anchorY: height - 14,
  rotates: false,
  frames: 1,
  draw,
});

export const SCENERY: SceneryDesign[] = [
  tree("tree_lollipop_pink", "Pink curly lollipop tree", () => drawLollipopTree("tree_lollipop_pink", COLORS.leafPink, "#d94a78", 15, 26, 1)),
  tree("tree_lollipop_yellow", "Yellow curly lollipop tree", () => drawLollipopTree("tree_lollipop_yellow", COLORS.leafYellow, "#c97f15", 17, 28, -1)),
  tree("tree_lollipop_green", "Small green curly lollipop tree", () => drawLollipopTree("tree_lollipop_green", COLORS.leafGreen, "#23865a", 12.5, 18, 1), 92),
  tree("tree_lollipop_teal", "Tall teal curly lollipop tree", () => drawLollipopTree("tree_lollipop_teal", COLORS.leafTeal, "#16705f", 14, 40, -1), 118),
  tree("tree_cloud", "Puffy cloud tree with curled leaves", () => drawCloudTree("tree_cloud", COLORS.leafGreen, "#23865a"), 96),
  tree("tree_cloud_blossom", "Puffy pink blossom tree", () => drawCloudTree("tree_cloud_blossom", COLORS.leafPink, "#d94a78"), 96),
  tree("tree_pine", "Three-tier swirly pine with a yellow tip", () => drawPine("tree_pine", COLORS.leafTeal), 102),
  { id: "bush_round", description: "Round curly bush", width: 54, height: 44, anchorY: 32, rotates: false, frames: 1, draw: () => drawBush("bush_round", COLORS.leafGreen, undefined) },
  { id: "bush_flowering", description: "Bush with pink flowers", width: 54, height: 44, anchorY: 32, rotates: false, frames: 1, draw: () => drawBush("bush_flowering", COLORS.leafTeal, COLORS.pink) },
  { id: "bench", description: "Red park bench with curled yellow ends", width: 64, height: 56, anchorY: 36, rotates: true, frames: 1, draw: drawBench },
  { id: "lamp", description: "Curly violet lamp post", width: 56, height: 78, anchorY: 66, rotates: true, frames: 1, draw: drawLamp },
  { id: "flower_bed", description: "Wooden flower bed with mixed blooms", width: 72, height: 62, anchorY: 42, rotates: true, frames: 1, draw: drawFlowerBed },
  { id: "fence", description: "Picket fence section along one tile edge", width: 72, height: 62, anchorY: 42, rotates: true, frames: 1, draw: drawFence },
  { id: "bin", description: "Smiling green bin with a yellow lid", width: 36, height: 44, anchorY: 34, rotates: false, frames: 1, draw: drawBin },
  { id: "fountain", description: "Two-tier fountain with moving water, one tile and a bit across", width: 92, height: 86, anchorY: 54, rotates: false, frames: 4, draw: drawFountain },
  { id: "signpost", description: "Signpost with RIDE and FOOD arrows", width: 60, height: 58, anchorY: 46, rotates: true, frames: 1, draw: drawSignpost },
  { id: "mushroom_stool", description: "Red spotted mushroom stool", width: 36, height: 36, anchorY: 27, rotates: false, frames: 1, draw: drawMushroomStool },
  { id: "balloon_cart", description: "Balloon seller's cart", width: 64, height: 104, anchorY: 88, rotates: true, frames: 1, draw: drawBalloonCart },
];
