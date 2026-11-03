import {
  fillPath,
  formatNumber,
  hullOfCircles,
  type Point,
  seededRandom,
  shadePlum,
  smoothPath,
  subdivide,
  svgDocument,
  wobble,
} from "./ink.ts";
import { depthOf, project, type Vector3 } from "./iso.ts";
import { USUAL_GAIT, type HairStyle, type PersonDesign } from "./person-design.ts";
import type { PeopleStyle, Theme } from "./theme.ts";

export type Facing = "se" | "sw" | "ne" | "nw";

/** Heading of each direction in view space, where +x is screen south-east and +y screen south-west. */
const HEADINGS: Record<Facing, readonly [number, number]> = {
  se: [1, 0],
  sw: [0, 1],
  nw: [-1, 0],
  ne: [0, -1],
};

export const WALK_FRAME_COUNT = 8;
export const IDLE_FRAME_COUNT = 2;
const OUTLINE_WIDTH = 1.5;

interface Ball {
  center: Vector3;
  radius: number;
}

/** One outlined silhouette made of several convex hulls. */
interface Layer {
  order: number;
  hulls: Ball[][];
  color: string;
  extra?: string;
  outline?: number;
}

export interface Pose {
  facing: Facing;
  animation: "walk" | "idle";
  frame: number;
}

/** The cast member's body in the theme's proportions. Hips stay where the theme puts them. */
function proportioned(person: PersonDesign, style: PeopleStyle): PersonDesign {
  const height = person.height * (person.isChild ? style.childHeightScale : style.heightScale);
  return {
    ...person,
    height,
    headRadius: person.headRadius * (person.isChild ? style.childHeadScale : style.headScale),
    limbRadius: person.limbRadius * style.limbScale,
    stride: person.stride * ((height * style.hipFraction) / (person.height * 0.44)),
  };
}

export function drawPerson(design: PersonDesign, { facing, animation, frame }: Pose, theme: Theme): string {
  const person = proportioned(design, theme.people);
  const [headingX, headingY] = HEADINGS[facing];
  const leftX = headingY;
  const leftY = -headingX;
  /** Body-local coordinates: forward, towards the person's left, up. */
  const toView = (forward: number, left: number, up: number): Vector3 => [
    headingX * forward + leftX * left,
    headingY * forward + leftY * left,
    up,
  ];
  const isWalking = animation === "walk";
  const gait = person.gait ?? USUAL_GAIT;
  const frameCount = isWalking ? WALK_FRAME_COUNT : IDLE_FRAME_COUNT;
  const phase = (frame / frameCount) * Math.PI * 2;
  const leftIsNear = depthOf(toView(0, 1, 0)) > 0;
  const facingViewer = depthOf(toView(1, 0, 0)) > 0;

  const sizeRatio = person.height / 43;
  const hipHeight = person.height * theme.people.hipFraction;
  const legLength = hipHeight - 1.4;
  const headCenterHeight = person.height - person.headRadius;
  const shoulderHeight = headCenterHeight - person.headRadius - 0.6;
  const bounce = isWalking ? -0.55 * sizeRatio * Math.cos(phase * 2) * gait.bounce : 0.22 * Math.sin(phase);
  const lean = isWalking ? 0.8 * sizeRatio : 0;

  const layers: Layer[] = [];
  const addLayer = (order: number, hulls: Ball[][], color: string, extra?: string, outline?: number): void => {
    layers.push({ order, hulls, color, extra, outline });
  };
  const ball = (forward: number, left: number, up: number, radius: number): Ball => ({
    center: toView(forward, left, up),
    radius,
  });
  const capsule = (start: Ball, end: Ball): Ball[] => [start, end];

  // Legs: planted feet slide backward under the body, then swing forward through the air.
  const legColor =
    person.lower === "shorts" || person.lower === "cargoShorts" || person.lower === "dress" ? person.skin : person.lowerColor;
  for (const side of [1, -1]) {
    const legPhase = phase + (side === 1 ? 0 : Math.PI);
    const footForward = isWalking ? person.stride * Math.cos(legPhase) : 0;
    const footLift = isWalking ? 3.4 * sizeRatio * gait.footLift * Math.max(0, -Math.sin(legPhase)) ** 1.3 : 0;
    const hipLeft = side * person.hipHalfWidth * 0.62;
    const hip = ball(0, hipLeft, hipHeight + bounce, person.limbRadius * 1.15);
    const ankle = ball(footForward, hipLeft, 1.8 + footLift, person.limbRadius * 0.82);
    const reach = Math.hypot(footForward, hipHeight + bounce - 1.8 - footLift);
    const kneeBend = Math.sqrt(Math.max(0, (legLength * 1.03) ** 2 - reach ** 2)) * 0.4;
    const knee = ball(
      footForward * 0.5 + kneeBend,
      hipLeft,
      (hipHeight + bounce + 1.8 + footLift) / 2,
      person.limbRadius,
    );
    const isNear = (side === 1) === leftIsNear;
    const order = isNear ? 2 : 1;
    const toe = ball(footForward + 2.6 * sizeRatio ** 0.5, hipLeft, 1.2 + footLift * 0.7, person.limbRadius * 0.85);
    const heel = ball(footForward - 0.5, hipLeft, 1.4 + footLift * 1.05, person.limbRadius * 0.85);
    if (person.lower === "shorts") {
      addLayer(order, [capsule(hip, knee), capsule(knee, ankle)], legColor);
      const hem: Ball = { center: lerpVector(hip.center, knee.center, 0.75), radius: person.limbRadius * 1.15 };
      addLayer(order + 0.1, [capsule(hip, hem)], person.lowerColor);
    } else if (person.lower === "cargoShorts") {
      // Baggy shorts down to the knee, with a patch pocket on the outside of each thigh.
      addLayer(order, [capsule(hip, knee), capsule(knee, ankle)], legColor);
      const hem: Ball = { center: lerpVector(hip.center, knee.center, 0.96), radius: person.limbRadius * 1.3 };
      addLayer(order + 0.1, [capsule({ ...hip, radius: person.limbRadius * 1.3 }, hem)], person.lowerColor);
      const outward = toView(0, side * person.limbRadius * 1.05, 0);
      const pocketAt = (fraction: number): Vector3 => {
        const onThigh = lerpVector(hip.center, knee.center, fraction);
        return [onThigh[0] + outward[0], onThigh[1] + outward[1], onThigh[2]];
      };
      addLayer(order + 0.12, [[{ center: pocketAt(0.42), radius: person.limbRadius * 0.62 }, { center: pocketAt(0.72), radius: person.limbRadius * 0.62 }]], person.pocketColor ?? person.lowerColor, undefined, 0.7);
    } else {
      addLayer(order, [capsule(hip, knee), capsule(knee, ankle)], legColor);
    }
    if (person.socks) addLayer(order + 0.15, [capsule({ ...ankle, radius: person.limbRadius * 0.9 }, { center: lerpVector(ankle.center, knee.center, 0.28), radius: person.limbRadius * 0.9 })], person.socks, undefined, 0.8);
    addLayer(order + 0.2, [[heel, toe]], person.shoes);
  }

  // Skirt and dress hems sway a little with the stride.
  const torsoLean = (height: number): number => lean * ((height - hipHeight) / (headCenterHeight - hipHeight));
  /** The stoop bends the upper back, so it grows towards the shoulders. */
  const stoopAt = (height: number): number => gait.stoop * Math.max(0, (height - hipHeight) / (headCenterHeight - hipHeight)) ** 1.6;
  const hemSway = isWalking ? Math.cos(phase) * 0.5 : 0;
  if (person.lower === "skirt" || person.lower === "dress") {
    const hemHeight = person.lower === "skirt" ? 5.5 * sizeRatio : hipHeight * 0.52;
    const hemRadius = person.hipHalfWidth + (person.lower === "skirt" ? 1.7 : 2.8) + person.belly * 0.3;
    const hemBalls = Array.from({ length: 10 }, (_, index) => {
      const angle = (index / 10) * Math.PI * 2;
      return ball(Math.cos(angle) * hemRadius + hemSway, Math.sin(angle) * hemRadius, hemHeight + bounce * 0.5, 0.7);
    });
    const waist = ball(torsoLean(hipHeight + 4) + person.belly * 0.4, 0, hipHeight + 4 + bounce, person.hipHalfWidth + person.belly * 0.5);
    addLayer(3, [[waist, ...hemBalls]], person.lowerColor);
  }
  if (person.tunic) {
    const hemRadius = person.hipHalfWidth + 1.2 + person.belly * 0.3;
    const hemBalls = Array.from({ length: 10 }, (_, index) => {
      const angle = (index / 10) * Math.PI * 2;
      return ball(Math.cos(angle) * hemRadius + hemSway * 0.6, Math.sin(angle) * hemRadius, hipHeight * 0.58 + bounce * 0.7, 0.7);
    });
    const waist = ball(torsoLean(hipHeight + 4) + stoopAt(hipHeight + 4) + person.belly * 0.4, 0, hipHeight + 4 + bounce, person.hipHalfWidth + person.belly * 0.5);
    addLayer(3.95, [[waist, ...hemBalls]], person.tunic);
  }

  // Torso: shoulders and hips joined by one hull, with a belly and chest pushing it forward.
  const shoulderZ = shoulderHeight + bounce;
  const breath = isWalking ? 0 : 0.2 * Math.sin(phase);
  const shoulderForward = torsoLean(shoulderHeight) + stoopAt(shoulderHeight);
  const leftShoulder = ball(shoulderForward, person.shoulderHalfWidth * 0.72, shoulderZ + breath, 1.9 * sizeRatio ** 0.5);
  const rightShoulder = ball(shoulderForward, -person.shoulderHalfWidth * 0.72, shoulderZ + breath, 1.9 * sizeRatio ** 0.5);
  const hipCore = hipHeight + bounce + 1.5;
  const torsoBalls: Ball[] = [
    leftShoulder,
    rightShoulder,
    ball(person.chest + stoopAt(shoulderHeight - 2.5), 0, shoulderZ - 2.5, person.shoulderHalfWidth * 0.8),
    ball(person.belly + stoopAt(hipCore + 3), 0, hipCore + 3, person.hipHalfWidth + person.belly * 0.55),
    ball(0, person.hipHalfWidth * 0.5, hipCore, 2.4),
    ball(0, -person.hipHalfWidth * 0.5, hipCore, 2.4),
  ];
  const needsLowerTorso = person.lower === "trousers" || person.lower === "shorts" || person.lower === "cargoShorts" || person.lower === "leggings";
  addLayer(4, [torsoBalls], person.top);
  if (needsLowerTorso) {
    const beltHeight = hipCore + 1.2;
    const pelvis: Ball[] = [
      ball(person.belly * 0.2, person.hipHalfWidth * 0.55, beltHeight - person.belly * 0.5, 2.3),
      ball(person.belly * 0.2, -person.hipHalfWidth * 0.55, beltHeight - person.belly * 0.5, 2.3),
      ball(0, person.hipHalfWidth * 0.6, hipHeight + bounce - 0.3, person.limbRadius * 1.2),
      ball(0, -person.hipHalfWidth * 0.6, hipHeight + bounce - 0.3, person.limbRadius * 1.2),
    ];
    addLayer(3.9, [pelvis], person.lowerColor);
  }
  if (person.overallStraps) {
    const bibTop = shoulderZ - 3;
    addLayer(4.3, [[
      ball(facingViewer ? 1.5 : -1.5, 1.4, bibTop, 1),
      ball(facingViewer ? 1.5 : -1.5, -1.4, bibTop, 1),
      ball(facingViewer ? 1 : -1, 1.8, hipCore + 1.5, 1.2),
      ball(facingViewer ? 1 : -1, -1.8, hipCore + 1.5, 1.2),
    ]], person.lowerColor, undefined, 0.6);
  }
  let stripeMarkup = "";
  if (person.topStripe) {
    const stripeLines: string[] = [];
    const clipId = "torso";
    const torsoOutline = hullOfCircles(torsoBalls.map(screenCircle));
    const shoulderWidth = person.shoulderHalfWidth * 2.2;
    const stripeHeights = [shoulderHeight - 2, shoulderHeight - 6, shoulderHeight - 10].filter((height) => height > hipHeight + 1);
    for (const stripeHeight of stripeHeights) {
      const start = project([...toFlat(toView(0, shoulderWidth, 0)), stripeHeight + bounce]);
      const end = project([...toFlat(toView(0, -shoulderWidth, 0)), stripeHeight + bounce]);
      stripeLines.push(`<path d="M${formatPoint(start)}L${formatPoint(end)}" stroke="${theme.color(person.topStripe)}" stroke-width="2"/>`);
    }
    stripeMarkup =
      `<clipPath id="${clipId}"><path d="${smoothPath(torsoOutline, true)}"/></clipPath>` +
      `<g clip-path="url(#${clipId})">${stripeLines.join("")}</g>`;
  }
  if (stripeMarkup) layers[layers.length - 1].extra = (layers[layers.length - 1].extra ?? "") + stripeMarkup;
  if (person.coasterPrint) {
    const torsoLayer = layers.find((layer) => layer.order === 4);
    if (!torsoLayer) throw new Error("The coaster print needs the torso layer");
    const printCenterUp = (shoulderZ + hipCore) / 2 + 0.6;
    torsoLayer.extra = (torsoLayer.extra ?? "") + coasterPrintMarkup(person, theme, torsoBalls, toView, printCenterUp, torsoLean(printCenterUp));
  }
  if (person.openFront && facingViewer) {
    const torsoLayer = layers.find((layer) => layer.order === 4);
    if (!torsoLayer) throw new Error("The open front needs the torso layer");
    torsoLayer.extra = (torsoLayer.extra ?? "") + openFrontMarkup(person, theme, torsoBalls, toView, shoulderZ, hipCore, (height) => torsoLean(height) + stoopAt(height));
  }

  // Arms swing against the legs on the same side.
  // A walking stick in the right hand moves with the left foot, planted while that foot is down.
  const caneTipForward = isWalking ? person.stride * Math.cos(phase) + 1.4 : 2.2;
  const caneTipLift = isWalking ? 2 * sizeRatio * Math.max(0, -Math.sin(phase)) ** 1.3 : 0;
  let carryingHand: Ball | undefined;
  for (const side of [1, -1]) {
    const armPhase = phase + (side === 1 ? Math.PI : 0);
    const swing = isWalking ? person.stride * 0.72 * gait.armSwing * Math.cos(armPhase) : 0;
    const isNear = (side === 1) === leftIsNear;
    const shoulderLeft = side * (person.shoulderHalfWidth * 0.86 + person.belly * 0.12);
    const shoulder = ball(shoulderForward, shoulderLeft, shoulderZ + breath - 0.3, person.limbRadius * 0.95);
    const armLength = (shoulderHeight - hipHeight) * 1.0;
    const carriesAccessory = side === -1 && (person.accessory === "iceCream" || person.accessory === "balloon" || person.accessory === "drink");
    const holdsCane = side === -1 && person.accessory === "walkingStick";
    const handForward = holdsCane
      ? caneTipForward * 0.45 + 0.8 + stoopAt(shoulderHeight) * 0.5
      : carriesAccessory ? 3.2 * sizeRatio ** 0.5 : shoulderForward + swing * 0.95 + person.belly * 0.5;
    const handUp = holdsCane
      ? hipHeight + bounce + 1 + caneTipLift * 0.6
      : carriesAccessory
        ? shoulderZ - armLength * 0.74
        : shoulderZ - armLength + Math.abs(swing) * 0.22 + 0.5;
    const handOut = shoulderLeft + side * ((carriesAccessory ? 3.2 : holdsCane ? 2.4 : 1.5) + person.belly * 0.4);
    const elbow = ball(
      (shoulderForward + handForward) * 0.42 - 0.3,
      (shoulderLeft + handOut) * 0.5 + side * 0.5,
      (shoulderZ + handUp) * 0.5,
      person.limbRadius * 0.9,
    );
    const hand = ball(handForward, handOut, handUp, person.limbRadius * 0.92);
    const order = isNear ? 6 : 0.5;
    const sleeveColor = person.sleeves === "long" ? (person.sleeveColor ?? person.top) : person.skin;
    if (person.sleeves === "short") {
      addLayer(order, [capsule(shoulder, elbow), capsule(elbow, hand)], person.skin);
      const sleeveEnd: Ball = { center: lerpVector(shoulder.center, elbow.center, 0.55), radius: person.limbRadius * 1.05 };
      addLayer(order + 0.1, [capsule(shoulder, sleeveEnd)], person.top);
    } else {
      addLayer(order, [capsule(shoulder, elbow), capsule(elbow, hand)], sleeveColor);
      addLayer(order + 0.1, [[{ ...hand, radius: hand.radius * 0.95 }]], person.skin);
    }
    if (carriesAccessory) carryingHand = hand;
    if (holdsCane) {
      const tip = ball(caneTipForward, handOut + side * 0.4, 0.5 + caneTipLift, 0.4);
      const top = ball(handForward, handOut, handUp + 1, 0.42);
      const crook = ball(handForward + 1.1, handOut, handUp + 1.5, 0.4);
      const crookEnd = ball(handForward + 1.9, handOut, handUp + 0.7, 0.38);
      addLayer(isNear ? 5.9 : 0.4, [capsule(tip, top), capsule(top, crook), capsule(crook, crookEnd)], person.accessoryColor, undefined, 0.8);
    }
  }

  // Head and hair.
  const headLean = lean * 1.2 + (isWalking ? Math.cos(phase * 2) * -0.05 : 0) + stoopAt(headCenterHeight) * 1.25;
  const headUp = headCenterHeight + bounce * 1.1 + breath * 0.5 - gait.stoop * 0.45;
  const headForward = headLean;
  const headRadius = person.headRadius;
  const headBall = (forward: number, left: number, up: number, radiusFactor: number): Ball =>
    ball(headForward + forward * headRadius, left * headRadius, headUp + up * headRadius, radiusFactor * headRadius);

  addLayer(8, [[headBall(0, 0, 0, 1)]], person.skin);
  const hairStyle = person.hairStyle;
  const addEars = hairStyle !== "hijab" && hairStyle !== "afro" && hairStyle !== "wavyBob";
  if (addEars) {
    for (const side of [1, -1]) {
      const isNear = (side === 1) === leftIsNear;
      addLayer(isNear ? 8.05 : 7.9, [[headBall(-0.05, side * 0.98, 0.02, 0.16)]], person.skin);
    }
  }
  const hairLayers = hairBallsFor(hairStyle);
  for (const hairLayer of hairLayers) {
    const balls = hairLayer.balls.map(([forward, left, up, radius]) => headBall(forward, left, up, radius));
    const meanForward = hairLayer.balls.reduce((sum, [forward]) => sum + forward, 0) / hairLayer.balls.length;
    const isBehindHead = hairLayer.fixedBehind ?? (facingViewer ? meanForward < -0.05 : meanForward > 0.3);
    const inFront = hairLayer.alwaysOnTop || !isBehindHead;
    addLayer(inFront ? 9 + hairLayer.bias : 7.5 + hairLayer.bias, [balls], person.hair);
  }
  if (person.beard) {
    addLayer(8.5, [[headBall(0.6, 0.45, -0.48, 0.5), headBall(0.6, -0.45, -0.48, 0.5), headBall(0.78, 0, -0.75, 0.42)]], person.beard);
  }
  if (person.hat === "cap") {
    // Worn backwards, so the peak points behind the head.
    addLayer(10, [[headBall(-0.3, 0, 0.5, 0.82)]], person.hatColor);
    addLayer(10.1, [[headBall(-0.8, 0.36, 0.36, 0.16), headBall(-0.8, -0.36, 0.36, 0.16), headBall(-1.3, 0.28, 0.28, 0.13), headBall(-1.3, -0.28, 0.28, 0.13)]], shadePlum(person.hatColor, 0.2));
  }
  if (person.hat === "beanie") {
    addLayer(10, [[headBall(-0.28, 0, 0.5, 0.84)]], person.hatColor);
    addLayer(10.1, [[headBall(-0.28, 0, 1.38, 0.25)]], "#fffaf0");
  }
  if (person.hat === "sunhat") {
    const brim = Array.from({ length: 12 }, (_, index) => {
      const angle = (index / 12) * Math.PI * 2;
      return headBall(Math.cos(angle) * 1.55, Math.sin(angle) * 1.55, 0.42, 0.1);
    });
    addLayer(10, [brim], person.hatColor);
    const hatBand = Array.from({ length: 10 }, (_, index) => {
      const angle = (index / 10) * Math.PI * 2;
      return headBall(Math.cos(angle) * 0.72, Math.sin(angle) * 0.72, 0.68, 0.12);
    });
    addLayer(10.1, [hatBand], "#ff5d5d", undefined, 0.7);
    addLayer(10.2, [[headBall(0, 0, 1.02, 0.56)]], person.hatColor);
  }

  // Back-mounted and carried things.
  if (person.accessory === "backpack") {
    const pack = [ball(-3.4, 2.2, shoulderZ - 1, 2), ball(-3.4, -2.2, shoulderZ - 1, 2), ball(-3.6, 2.2, hipCore + 2, 2.2), ball(-3.6, -2.2, hipCore + 2, 2.2)];
    addLayer(facingViewer ? 3.5 : 5, [pack], person.accessoryColor);
    if (!facingViewer) addLayer(5.1, [[ball(-5.4, 0, hipCore + 3, 1.5), ball(-5.4, 1, hipCore + 3, 1.5)]], shadePlum(person.accessoryColor, 0.22), undefined, 0.8);
  }
  if (person.accessory === "satchel" || person.accessory === "handbag") {
    const side = person.accessory === "satchel" ? 1 : -1;
    const isNear = (side === 1) === leftIsNear;
    const bagHeight = hipCore - 0.5;
    const swingOffset = isWalking ? Math.cos(phase + Math.PI) * 0.8 : 0;
    const bagLeft = side * (person.hipHalfWidth + 3.3 + person.belly * 0.4);
    const bag = [
      ball(swingOffset - 1.4, bagLeft, bagHeight + 0.6, 1.5),
      ball(swingOffset + 1.4, bagLeft, bagHeight + 0.6, 1.5),
      ball(swingOffset - 1.1, bagLeft, bagHeight - 1.5, 1.7),
      ball(swingOffset + 1.1, bagLeft, bagHeight - 1.5, 1.7),
    ];
    addLayer(isNear ? 6.3 : 0.2, [bag], person.accessoryColor);
  }
  if (person.accessory === "tote") {
    // A tall canvas bag hanging from the left shoulder by a long strap.
    const isNear = leftIsNear;
    const swingOffset = isWalking ? Math.cos(phase + Math.PI) * 0.6 : 0;
    const bagLeft = person.hipHalfWidth + 2.6 + person.belly * 0.4;
    const bagTop = hipCore + 1.6;
    const bag = [
      ball(swingOffset - 1.5, bagLeft, bagTop, 1.1),
      ball(swingOffset + 1.5, bagLeft, bagTop, 1.1),
      ball(swingOffset - 1.6, bagLeft, bagTop - 4, 1.2),
      ball(swingOffset + 1.6, bagLeft, bagTop - 4, 1.2),
    ];
    const strapTop = ball(shoulderForward, person.shoulderHalfWidth * 0.62, shoulderZ + 1.2, 0.4);
    const straps = [capsule(strapTop, ball(swingOffset - 1.1, bagLeft, bagTop, 0.4)), capsule(strapTop, ball(swingOffset + 1.1, bagLeft, bagTop, 0.4))];
    addLayer(isNear ? 6.3 : 0.2, [...straps, bag], person.accessoryColor);
  }
  if (person.accessory === "camera") {
    const chestForward = person.chest + person.shoulderHalfWidth * 0.8 + 0.2 + (person.belly > 0 ? 0.4 : 0);
    if (facingViewer) {
      const lens = project(toView(chestForward + 1, 0, shoulderZ - 3.8));
      const lensMarkup = `<circle cx="${formatNumber(lens[0])}" cy="${formatNumber(lens[1])}" r="0.9" fill="${theme.color("#a9dcf2")}"/>`;
      addLayer(5.5, [[ball(chestForward, 1.1, shoulderZ - 3.6, 1.05), ball(chestForward, -1.1, shoulderZ - 3.6, 1.05)]], "#5a4a78", lensMarkup, 0.8);
    }
  }
  if (person.accessory === "beltBag") {
    // Worn across the body, from the right shoulder to the left hip, with the pouch on the chest.
    // The strap shows on whichever side of the torso is seen; the pouch only from the front.
    const seenSide = facingViewer ? 1 : -1;
    const chestFront = person.chest + person.shoulderHalfWidth * 0.8;
    const bellyFront = person.belly + person.hipHalfWidth + person.belly * 0.55;
    const strapTop = ball(shoulderForward + seenSide * chestFront * 0.55, -person.shoulderHalfWidth * 0.62, shoulderZ + 0.4, 0.42);
    const strapBottom = ball(seenSide * bellyFront * 0.75, person.hipHalfWidth * 0.95, hipCore + 1.2, 0.42);
    addLayer(4.4, [capsule(strapTop, strapBottom)], shadePlum(person.accessoryColor, 0.3), undefined, 0.6);
    if (facingViewer) {
      const pouchUp = shoulderZ - (shoulderZ - hipCore) * 0.42;
      const pouchForward = person.chest * 0.5 + person.belly * 0.5 + person.shoulderHalfWidth * 0.78 + stoopAt(pouchUp);
      addLayer(4.5, [[ball(pouchForward, 1.5, pouchUp, 1.05), ball(pouchForward, -1.3, pouchUp + 0.6, 1.05)]], person.accessoryColor, undefined, 0.8);
    }
  }

  // Composite in layer order.
  layers.sort((first, second) => first.order - second.order);
  const random = seededRandom(`${person.id}/${facing}`);
  const ink = theme.line.color;
  const body = layers
    .map((layer) => {
      const outlines = layer.hulls.map((hull) => wobblyHull(hull, random, theme.shapeWobble));
      const width = (layer.outline ?? OUTLINE_WIDTH) * theme.people.outline;
      const inkPass =
        width > 0
          ? outlines
              .map((outline) => `<path d="${outline}" fill="${ink}" stroke="${ink}" stroke-width="${width}" stroke-linejoin="round"/>`)
              .join("")
          : "";
      const colorPass = outlines.map((outline) => fillPath(theme, outline, theme.color(layer.color))).join("");
      return inkPass + colorPass + (layer.extra ?? "");
    })
    .join("\n");

  const face =
    (facingViewer ? drawFace(person, theme, headBall, leftIsNear ? 1 : -1, animation === "idle" && frame === 1) : "") +
    (facingViewer && person.moustache && theme.people.face !== "candy" ? drawSharedMoustache(person, theme, headBall, leftIsNear ? 1 : -1) : "") +
    (person.glassesStrap ? drawStrappedGlasses(person, theme, headBall, leftIsNear ? 1 : -1, facingViewer) : "") +
    (facingViewer && (person.framedGlasses || (person.glasses && theme.people.face !== "candy")) ? drawFramedLenses(person, theme, headBall, leftIsNear ? 1 : -1) : "");

  // Things drawn above everything else, for a held balloon, ice cream or drink.
  let held = "";
  if (carryingHand && person.accessory === "balloon") {
    const [handX, handY] = project(carryingHand.center);
    const bob = isWalking ? Math.sin(phase) * 0.6 : Math.sin(phase) * 0.4;
    const balloonX = handX + 6 * (facing === "sw" || facing === "nw" ? -0.3 : 0.3);
    const balloonY = handY - 29 + bob;
    const color = theme.color(person.accessoryColor);
    const highlightOpacity = 0.75 * theme.highlight.opacity;
    held =
      `<path d="M${formatNumber(handX)} ${formatNumber(handY)}C${formatNumber(handX + 2)} ${formatNumber(handY - 10)} ${formatNumber(balloonX - 2)} ${formatNumber(balloonY + 15)} ${formatNumber(balloonX)} ${formatNumber(balloonY + 8)}" fill="none" stroke="${ink}" stroke-width="${0.7 * theme.line.weight}"/>` +
      `<path d="M${formatNumber(balloonX - 1.4)} ${formatNumber(balloonY + 8.6)}L${formatNumber(balloonX)} ${formatNumber(balloonY + 6.5)}L${formatNumber(balloonX + 1.4)} ${formatNumber(balloonY + 8.6)}Z" fill="${ink}"/>` +
      `<ellipse cx="${formatNumber(balloonX)}" cy="${formatNumber(balloonY)}" rx="6.2" ry="7.6" fill="${color}" stroke="${ink}" stroke-width="${1.3 * theme.people.outline}"/>` +
      (highlightOpacity > 0 ? `<ellipse cx="${formatNumber(balloonX - 2)}" cy="${formatNumber(balloonY - 2.6)}" rx="1.4" ry="2.1" fill="${theme.highlight.color}" fill-opacity="${highlightOpacity}"/>` : "");
  }
  if (carryingHand && person.accessory === "iceCream") {
    const [handX, handY] = project(carryingHand.center);
    const coneTop = handY - 2.6;
    held =
      `<path d="M${formatNumber(handX - 2.1)} ${formatNumber(coneTop)}L${formatNumber(handX + 2.1)} ${formatNumber(coneTop)}L${formatNumber(handX)} ${formatNumber(handY + 2)}Z" fill="${theme.color("#e8a06f")}" stroke="${ink}" stroke-width="${0.9 * theme.people.outline}" stroke-linejoin="round"/>` +
      `<circle cx="${formatNumber(handX)}" cy="${formatNumber(coneTop - 1.6)}" r="2.5" fill="${theme.color(person.accessoryColor)}" stroke="${ink}" stroke-width="${0.9 * theme.people.outline}"/>`;
  }
  if (carryingHand && person.accessory === "drink") {
    // A tall paper cup held round the middle, with a domed lid and a striped straw.
    const [handX, handY] = project(carryingHand.center);
    const [cupTop, cupBottom] = [handY - 3.6, handY + 2.2];
    const strokeWidth = formatNumber(0.9 * theme.people.outline);
    const strawLean = facing === "sw" || facing === "nw" ? -1 : 1;
    held =
      `<path d="M${formatNumber(handX + 0.4 * strawLean)} ${formatNumber(cupTop - 0.6)}L${formatNumber(handX + 1.5 * strawLean)} ${formatNumber(cupTop - 4)}" stroke="${ink}" stroke-width="${formatNumber(1.1 + 2 * 0.6 * theme.people.outline)}" stroke-linecap="round"/>` +
      `<path d="M${formatNumber(handX + 0.4 * strawLean)} ${formatNumber(cupTop - 0.6)}L${formatNumber(handX + 1.5 * strawLean)} ${formatNumber(cupTop - 4)}" stroke="${theme.color("#ff5d5d")}" stroke-width="1.1" stroke-linecap="round"/>` +
      `<path d="M${formatNumber(handX - 2.2)} ${formatNumber(cupTop)}L${formatNumber(handX + 2.2)} ${formatNumber(cupTop)}L${formatNumber(handX + 1.6)} ${formatNumber(cupBottom)}L${formatNumber(handX - 1.6)} ${formatNumber(cupBottom)}Z" fill="${theme.color(person.accessoryColor)}" stroke="${ink}" stroke-width="${strokeWidth}" stroke-linejoin="round"/>` +
      `<path d="M${formatNumber(handX - 2.5)} ${formatNumber(cupTop + 0.2)}Q${formatNumber(handX)} ${formatNumber(cupTop - 2)} ${formatNumber(handX + 2.5)} ${formatNumber(cupTop + 0.2)}Z" fill="${theme.color("#fffaf0")}" stroke="${ink}" stroke-width="${strokeWidth}" stroke-linejoin="round"/>`;
  }

  // Contact shadow, drawn with the sprite so that it always stays beneath the feet.
  const shadowRadius = 5.8 + person.belly * 0.8 + (person.height - 25) * 0.05;
  const shadow = `<ellipse cx="0.6" cy="0.3" rx="${formatNumber(shadowRadius)}" ry="${formatNumber(shadowRadius * 0.46)}" fill="${theme.shadow.color}" fill-opacity="${0.26 * theme.shadow.opacity}"/>`;
  return `${shadow}\n${body}\n${face}\n${held}`;
}

function drawFace(
  person: PersonDesign,
  theme: Theme,
  headBall: (forward: number, left: number, up: number, radiusFactor: number) => Ball,
  nearSide: number,
  blinking: boolean,
): string {
  if (theme.people.face === "none") return "";
  const ink = theme.line.color;
  const parts: string[] = [];
  const onHead = (forward: number, left: number, up: number): Point => project(onFace(headBall, nearSide, forward, left, up));
  const eyeUp = eyeHeight(person);
  const leftEye = onHead(0.86, 0.46, eyeUp);
  const rightEye = onHead(0.86, -0.46, eyeUp);
  const eyes = [leftEye, rightEye];
  if (theme.people.face === "dots") {
    // Two small dots and nothing else, as in a quick sketch of a person seen from a little way off.
    const dotRadius = Math.max(0.42, person.headRadius * 0.12);
    return eyes.map(([eyeX, eyeY]) => `<circle cx="${formatNumber(eyeX)}" cy="${formatNumber(eyeY)}" r="${formatNumber(dotRadius)}" fill="${ink}"/>`).join("");
  }
  const eyeRadius = person.headRadius * 0.15;
  for (const [eyeX, eyeY] of eyes) {
    parts.push(
      blinking
        ? `<path d="M${formatNumber(eyeX - eyeRadius)} ${formatNumber(eyeY)}h${formatNumber(eyeRadius * 2)}" stroke="${ink}" stroke-width="0.7"/>`
        : `<ellipse cx="${formatNumber(eyeX)}" cy="${formatNumber(eyeY)}" rx="${formatNumber(eyeRadius)}" ry="${formatNumber(eyeRadius * 1.3)}" fill="${ink}"/>`,
    );
  }
  // Rosy cheeks are visible on every skin tone because they use an orange-pink, not a fixed tint of the skin.
  const cheekOpacity = 0.45;
  for (const cheek of [onHead(0.8, 0.68, -0.28), onHead(0.8, -0.68, -0.28)]) {
    parts.push(
      `<ellipse cx="${formatNumber(cheek[0])}" cy="${formatNumber(cheek[1])}" rx="${formatNumber(person.headRadius * 0.2)}" ry="${formatNumber(person.headRadius * 0.14)}" fill="${theme.color("#ff5d6d")}" fill-opacity="${cheekOpacity}"/>`,
    );
  }
  // Smile: a short arc on the lower face.
  const smileStart = onHead(0.82, 0.3, -0.38);
  const smileMiddle = onHead(0.86, 0, -0.56);
  const smileEnd = onHead(0.82, -0.3, -0.38);
  const smileColor = person.beard ? theme.color("#6b2a3a") : ink;
  parts.push(
    `<path d="M${formatPoint(smileStart)}Q${formatPoint(smileMiddle)} ${formatPoint(smileEnd)}" fill="none" stroke="${smileColor}" stroke-width="0.75" stroke-linecap="round"/>`,
  );
  if (person.moustache && theme.people.face === "candy") {
    const moustacheLeft = onHead(0.82, 0.42, -0.34);
    const moustacheMiddle = onHead(0.9, 0, -0.14);
    const moustacheRight = onHead(0.82, -0.42, -0.34);
    parts.push(
      `<path d="M${formatPoint(moustacheLeft)}Q${formatPoint(moustacheMiddle)} ${formatPoint(moustacheRight)}" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>` +
        `<path d="M${formatPoint(moustacheLeft)}Q${formatPoint(moustacheMiddle)} ${formatPoint(moustacheRight)}" fill="none" stroke="${theme.color(person.moustache)}" stroke-width="1.6" stroke-linecap="round"/>`,
    );
  }
  if (person.glasses) {
    const frame = theme.color(person.glasses);
    for (const [eyeX, eyeY] of eyes) {
      parts.push(`<circle cx="${formatNumber(eyeX)}" cy="${formatNumber(eyeY)}" r="${formatNumber(person.headRadius * 0.3)}" fill="${theme.color("#ffffff")}" fill-opacity="0.35" stroke="${frame}" stroke-width="0.6"/>`);
    }
    parts.push(`<path d="M${formatPoint(leftEye)}L${formatPoint(rightEye)}" stroke="${frame}" stroke-width="0.5" fill="none"/>`);
    for (const [eyeX, eyeY] of eyes) {
      parts.push(`<circle cx="${formatNumber(eyeX)}" cy="${formatNumber(eyeY)}" r="${formatNumber(person.headRadius * 0.12)}" fill="${ink}"/>`);
    }
  }
  return parts.join("");
}

/** Moustaches for the simplified faces and the faceless mid-century silhouettes. */
function drawSharedMoustache(
  person: PersonDesign,
  theme: Theme,
  headBall: (forward: number, left: number, up: number, radiusFactor: number) => Ball,
  nearSide: number,
): string {
  if (!person.moustache) throw new Error(`${person.id} has no moustache`);
  const onHead = (forward: number, left: number, up: number): Point => project(onFace(headBall, nearSide, forward, left, up));
  const halfWidth = theme.people.face === "dots" ? 0.52 : 0.48;
  const moustacheLeft = onHead(0.82, halfWidth, -0.3);
  const moustacheMiddle = onHead(0.91, 0, -0.12);
  const moustacheRight = onHead(0.82, -halfWidth, -0.3);
  const moustacheBottom = onHead(0.82, 0, -0.58);
  const path = `M${formatPoint(moustacheLeft)}Q${formatPoint(moustacheMiddle)} ${formatPoint(moustacheRight)}Q${formatPoint(moustacheBottom)} ${formatPoint(moustacheLeft)}Z`;
  const fill = theme.color(person.moustache);
  if (theme.people.face === "none") return `<path d="${path}" fill="${fill}"/>`;
  return `<path d="${path}" fill="${fill}" stroke="${theme.line.color}" stroke-width="${formatNumber(theme.people.outline * 1.1)}" stroke-linejoin="round"/>`;
}

/** Face features are cheated towards the camera, as a character artist would: at a true three-quarter
 * angle seen from above, the far eye falls off the head and the face sits too low to read. */
const FACE_YAW_TOWARD_CAMERA = (24 * Math.PI) / 180;

/** A point on the head in head-radius units, turned with the face towards the camera. */
function onFace(
  headBall: (forward: number, left: number, up: number, radiusFactor: number) => Ball,
  nearSide: number,
  forward: number,
  left: number,
  up: number,
): Vector3 {
  return headBall(
    forward * Math.cos(FACE_YAW_TOWARD_CAMERA) - left * nearSide * Math.sin(FACE_YAW_TOWARD_CAMERA),
    left * Math.cos(FACE_YAW_TOWARD_CAMERA) + forward * nearSide * Math.sin(FACE_YAW_TOWARD_CAMERA),
    up + 0.2,
    0,
  ).center;
}

function eyeHeight(person: PersonDesign): number {
  return person.hat === "sunhat" ? 0.02 : 0.1;
}

/**
 * Glasses on a sports strap. The strap runs round the back of the head at eye height, so it reads
 * from behind as well; the lenses show only from the front, in every face style.
 */
function drawStrappedGlasses(
  person: PersonDesign,
  theme: Theme,
  headBall: (forward: number, left: number, up: number, radiusFactor: number) => Ball,
  nearSide: number,
  facingViewer: boolean,
): string {
  if (!person.glassesStrap) throw new Error(`${person.id} has no glasses strap`);
  const ink = theme.line.color;
  const outline = theme.people.outline;
  const eyeUp = eyeHeight(person);
  const headCenter = headBall(0, 0, 0, 0).center;
  // Seen from the front, the face is turned towards the camera; from behind it is not.
  const pointOnHead = (forward: number, left: number, up: number): Vector3 =>
    facingViewer ? onFace(headBall, nearSide, forward, left, up) : headBall(forward, left, up + 0.2, 0).center;
  const strapRadius = 1.04;
  const runs: Point[][] = [];
  let run: Point[] = [];
  for (let step = 0; step <= 48; step++) {
    const angle = (step / 48) * Math.PI * 2;
    const forward = Math.cos(angle) * strapRadius;
    // The strap hangs slack, dropping from the temples to the nape of the neck.
    const sag = 0.85 * Math.max(0, -Math.cos(angle)) ** 1.4;
    const point = pointOnHead(forward, Math.sin(angle) * strapRadius, eyeUp - sag);
    const isOnSeenSide = depthOf([point[0] - headCenter[0], point[1] - headCenter[1], point[2] - headCenter[2]]) > -0.15 * person.headRadius;
    // The front of the ring is the glasses themselves.
    if (isOnSeenSide && forward < 0.62) {
      run.push(project(point));
    } else if (run.length > 0) {
      runs.push(run);
      run = [];
    }
  }
  if (run.length > 0) runs.push(run);
  const strapPath = runs
    .filter((points) => points.length > 1)
    .map((points) => `M${points.map(formatPoint).join("L")}`)
    .join("");
  const parts = [
    `<path d="${strapPath}" fill="none" stroke="${ink}" stroke-width="${formatNumber(1.2 * outline + 0.35)}" stroke-linecap="round" stroke-linejoin="round"/>`,
    `<path d="${strapPath}" fill="none" stroke="${theme.color(person.glassesStrap)}" stroke-width="0.7" stroke-linecap="round" stroke-linejoin="round"/>`,
  ];
  if (facingViewer) parts.push(drawFramedLenses(person, theme, headBall, nearSide));
  return parts.join("");
}

/** Two lenses with an ink frame and the eyes behind them, bold enough to show in every face style. */
function drawFramedLenses(
  person: PersonDesign,
  theme: Theme,
  headBall: (forward: number, left: number, up: number, radiusFactor: number) => Ball,
  nearSide: number,
): string {
  const ink = theme.line.color;
  const eyeUp = eyeHeight(person);
  const eyes = [onFace(headBall, nearSide, 0.86, 0.46, eyeUp), onFace(headBall, nearSide, 0.86, -0.46, eyeUp)].map(project);
  const lensRadius = person.headRadius * 0.3;
  const frameWidth = formatNumber(Math.max(0.55, 0.75 * theme.people.outline));
  const parts: string[] = [];
  for (const [eyeX, eyeY] of eyes) {
    parts.push(`<circle cx="${formatNumber(eyeX)}" cy="${formatNumber(eyeY)}" r="${formatNumber(lensRadius)}" fill="${theme.color("#ffffff")}" fill-opacity="0.45" stroke="${ink}" stroke-width="${frameWidth}"/>`);
  }
  parts.push(`<path d="M${formatPoint(eyes[0])}L${formatPoint(eyes[1])}" stroke="${ink}" stroke-width="${frameWidth}" fill="none"/>`);
  if (theme.people.face !== "none") {
    const pupilRadius = theme.people.face === "dots" ? Math.max(0.42, person.headRadius * 0.12) : person.headRadius * 0.12;
    for (const [eyeX, eyeY] of eyes) parts.push(`<circle cx="${formatNumber(eyeX)}" cy="${formatNumber(eyeY)}" r="${formatNumber(pupilRadius)}" fill="${ink}"/>`);
  }
  return parts.join("");
}

/**
 * The shirt showing down the front of an open cardigan or jacket: a V at the neck narrowing to a
 * strip down to the hem, clipped to the torso. Only drawn from the front.
 */
function openFrontMarkup(
  person: PersonDesign,
  theme: Theme,
  torsoBalls: readonly Ball[],
  toView: (forward: number, left: number, up: number) => Vector3,
  shoulderUp: number,
  hipUp: number,
  forwardAt: (up: number) => number,
): string {
  if (!person.openFront) throw new Error(`${person.id} has no open front`);
  const chestFront = person.chest + person.shoulderHalfWidth * 0.8;
  const bellyFront = person.belly + person.hipHalfWidth + person.belly * 0.55;
  const surface = (up: number, left: number): Point => {
    const fraction = Math.min(1, Math.max(0, (shoulderUp - up) / (shoulderUp - hipUp)));
    const front = chestFront + (bellyFront - chestFront) * fraction;
    // Pulled in from the true surface, so that the opening sits near the middle of the torso as seen.
    return project(toView(forwardAt(up) + front * 0.6 * Math.sqrt(Math.max(0, 1 - (left / front) ** 2)), left, up));
  };
  const neckUp = shoulderUp + 1.2;
  const vBottomUp = shoulderUp - (shoulderUp - hipUp) * 0.42;
  const hemUp = hipUp - 1;
  const neckHalfWidth = person.shoulderHalfWidth * 0.36;
  const stripHalfWidth = Math.max(0.55, person.shoulderHalfWidth * 0.11);
  const outline = [
    surface(neckUp, neckHalfWidth),
    surface(vBottomUp, stripHalfWidth),
    surface(hemUp, stripHalfWidth),
    surface(hemUp, -stripHalfWidth),
    surface(vBottomUp, -stripHalfWidth),
    surface(neckUp, -neckHalfWidth),
  ];
  const clipId = "front";
  const torsoOutline = hullOfCircles(torsoBalls.map(screenCircle));
  const ink = theme.line.color;
  return (
    `<clipPath id="${clipId}"><path d="${smoothPath(torsoOutline, true)}"/></clipPath>` +
    `<g clip-path="url(#${clipId})"><path d="M${outline.map(formatPoint).join("L")}Z" fill="${theme.color(person.openFront)}" stroke="${ink}" stroke-width="${formatNumber(0.8 * theme.people.outline)}" stroke-linejoin="round"/></g>`
  );
}

/**
 * A roller coaster printed on the shirt: a lift hill, a drop and a loop, bold enough to read as a
 * coaster at game size, with supports and a little train that only show when zoomed in. The same
 * print is on the front and the back, so it shows from every direction; it is clipped to the torso.
 */
function coasterPrintMarkup(
  person: PersonDesign,
  theme: Theme,
  torsoBalls: readonly Ball[],
  toView: (forward: number, left: number, up: number) => Vector3,
  centerUp: number,
  centerForward: number,
): string {
  if (!person.coasterPrint) throw new Error(`${person.id} has no coaster print`);
  const size = person.shoulderHalfWidth * 0.72;
  // Like the face, the print is cheated towards the camera: it keeps only a third of the slope that
  // a flat print on the chest would have in this view, so it reads as a picture, not a ramp.
  const [centerX, centerY] = project(toView(centerForward, 0, centerUp));
  const sideways = project(toView(0, 1, 0));
  const slope = (sideways[1] / sideways[0]) * 0.3;
  const [acrossScale, upScale] = [size * 0.74, size * 0.78];
  /** Across runs from the viewer's left to right on whichever side is seen; up is up the shirt. */
  const onShirt = ([across, up]: readonly [number, number]): Point => [centerX + across * acrossScale, centerY + across * acrossScale * slope - up * upScale];
  // A straight lift hill, a steep drop and a round loop, all standing on one ground line.
  const groundLevel = -0.9;
  const track: [number, number][] = [[-1.4, groundLevel], [-0.32, 0.5], [-0.22, 0.58], [-0.13, 0.48], [-0.06, -0.62], [0.08, -0.84]];
  const [loopCenterAcross, loopRadius] = [0.66, 0.6];
  const loopBottom = -0.84;
  for (let step = 0; step <= 24; step++) {
    const angle = -Math.PI / 2 + (step / 24) * Math.PI * 2;
    const drift = (step / 24) * 0.1;
    track.push([loopCenterAcross + drift + Math.cos(angle) * loopRadius, loopBottom + loopRadius + Math.sin(angle) * loopRadius]);
  }
  track.push([1.2, loopBottom]);
  const supports = [[-1.1, -0.51], [-0.82, -0.14], [-0.54, 0.22]].map(([across, top]): [number, number][] => [[across, top], [across, groundLevel]]);
  const groundLine: [number, number][] = [[-1.25, groundLevel], [1.25, groundLevel]];
  const trainCars: [number, number][] = [[-0.72, 0.0], [-0.6, 0.15], [-0.48, 0.31]];
  const print = theme.color(person.coasterPrint);
  const clipId = "shirt";
  const torsoOutline = hullOfCircles(torsoBalls.map(screenCircle));
  const supportMarkup = supports
    .map((support) => `<path d="M${support.map(onShirt).map(formatPoint).join("L")}" stroke="${print}" stroke-width="0.3" stroke-opacity="0.8"/>`)
    .join("");
  const trainMarkup = trainCars
    .map((car) => {
      const [carX, carY] = onShirt(car);
      return `<circle cx="${formatNumber(carX)}" cy="${formatNumber(carY)}" r="0.5" fill="${theme.color("#ff5d5d")}" stroke="none"/>`;
    })
    .join("");
  return (
    `<clipPath id="${clipId}"><path d="${smoothPath(torsoOutline, true)}"/></clipPath>` +
    `<g clip-path="url(#${clipId})" fill="none" stroke-linecap="round" stroke-linejoin="round">` +
    supportMarkup +
    `<path d="M${groundLine.map(onShirt).map(formatPoint).join("L")}" stroke="${print}" stroke-width="0.5"/>` +
    `<path d="${smoothPath(track.map(onShirt), false)}" stroke="${print}" stroke-width="0.8"/>` +
    trainMarkup +
    `</g>`
  );
}

type HairBall = readonly [forward: number, left: number, up: number, radius: number];
interface HairLayer {
  balls: HairBall[];
  bias: number;
  alwaysOnTop?: boolean;
  fixedBehind?: boolean;
}

/** Hair is built from balls on the head, in head-radius units. */
function hairBallsFor(style: HairStyle): HairLayer[] {
  const backOfHead: HairBall = [-0.22, 0, 0.1, 0.98];
  switch (style) {
    case "afro":
      return [
        { balls: [[-0.2, 0, 0.42, 1.52]], bias: 0, fixedBehind: undefined },
        { balls: [[0.12, 0.5, 0.86, 0.48], [0.12, -0.5, 0.86, 0.48], [0.05, 0, 1, 0.52]], bias: 0.2, alwaysOnTop: true },
      ];
    case "shortCrop":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.78]], bias: 0.2, alwaysOnTop: true },
      ];
    case "swoop":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.78], [0.42, 0.45, 0.72, 0.34], [0.4, -0.28, 0.82, 0.28]], bias: 0.2, alwaysOnTop: true },
      ];
    case "bun":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.78]], bias: 0.1, alwaysOnTop: true },
        { balls: [[-0.38, 0, 1.28, 0.5]], bias: 0.3, alwaysOnTop: true },
      ];
    case "bald":
      return [];
    case "braid":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.78]], bias: 0.2, alwaysOnTop: true },
        { balls: [[-0.92, 0.1, -0.5, 0.34], [-1, 0.1, -1.2, 0.32], [-1, 0.1, -2, 0.28], [-0.95, 0.1, -2.6, 0.2]], bias: 0.1 },
      ];
    case "bowl":
      return [
        { balls: [[-0.08, 0, 0.14, 1.12]], bias: 0 },
        { balls: [[-0.16, 0, 0.56, 0.86], [0.4, 0.5, 0.62, 0.28], [0.4, -0.5, 0.62, 0.28], [0.46, 0, 0.72, 0.28]], bias: 0.2, alwaysOnTop: true },
      ];
    case "pigtails":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.78]], bias: 0.2, alwaysOnTop: true },
        { balls: [[-0.25, 1.15, 0.28, 0.46], [-0.25, 1.4, -0.3, 0.36]], bias: 0.1, alwaysOnTop: false },
        { balls: [[-0.25, -1.15, 0.28, 0.46], [-0.25, -1.4, -0.3, 0.36]], bias: 0.1, alwaysOnTop: false },
      ];
    case "puffs":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.76]], bias: 0.2, alwaysOnTop: true },
        { balls: [[-0.1, 0.82, 0.95, 0.56]], bias: 0.3, alwaysOnTop: true },
        { balls: [[-0.1, -0.82, 0.95, 0.56]], bias: 0.3, alwaysOnTop: true },
      ];
    case "hijab":
      return [
        { balls: [[-0.24, 0, 0.04, 1.16]], bias: 0 },
        { balls: [[-0.12, 0, -1.02, 0.68]], bias: 0.1 },
        { balls: [[-0.3, 0, 0.5, 0.8]], bias: 0.2, alwaysOnTop: true },
      ];
    case "wavyBob":
      // Chin length all round. The fringe goes behind the head when seen from the back, so the back of
      // the bob reads as one smooth shape.
      return [
        { balls: [[-0.24, 0, -0.12, 1.12], [-0.12, 0, 0.42, 0.98]], bias: 0 },
        { balls: [[0.34, 0, 0.56, 0.72], [0.36, 0.56, 0.42, 0.36], [0.36, -0.56, 0.42, 0.36]], bias: 0.2 },
      ];
    case "highBun":
      return [
        { balls: [backOfHead], bias: 0 },
        { balls: [[-0.3, 0, 0.5, 0.8]], bias: 0.1, alwaysOnTop: true },
        { balls: [[-0.2, 0, 1.36, 0.62]], bias: 0.3, alwaysOnTop: true },
      ];
    case "balding":
      // Bare on top, with a fringe round the back and over the ears.
      return [
        { balls: [[-0.42, 0, -0.12, 0.78], [-0.08, 0.7, 0.02, 0.42], [-0.08, -0.7, 0.02, 0.42]], bias: 0, fixedBehind: undefined },
      ];
  }
}

function wobblyHull(balls: readonly Ball[], random: () => number, wobbleScale: number): string {
  const outline = hullOfCircles(balls.map(screenCircle));
  const resampled = subdivide(outline.filter((_, index) => index % 2 === 0), true, 3);
  return smoothPath(wobble(resampled, 0.14 * wobbleScale, random, 9), true);
}

function screenCircle({ center, radius }: Ball): { center: Point; radius: number } {
  return { center: project(center), radius };
}

function lerpVector(first: Vector3, second: Vector3, fraction: number): Vector3 {
  return [
    first[0] + (second[0] - first[0]) * fraction,
    first[1] + (second[1] - first[1]) * fraction,
    first[2] + (second[2] - first[2]) * fraction,
  ];
}

function toFlat([x, y]: Vector3): [number, number] {
  return [x, y];
}

function formatPoint([x, y]: Point): string {
  return `${formatNumber(x)} ${formatNumber(y)}`;
}

/** The canvas of a person sprite is the same for all frames, so that the anchor stays fixed. */
export function personDocument(design: PersonDesign, pose: Pose, theme: Theme): string {
  const person = proportioned(design, theme.people);
  const extraHeadroom = person.accessory === "balloon" ? 30 : 0;
  const height = Math.ceil(person.height * Math.sqrt(3) * 0.5 + 20 + extraHeadroom);
  const width = 46;
  const anchorX = width / 2;
  const anchorY = height - 9;
  const body = drawPerson(design, pose, theme);
  return svgDocument(
    width,
    height,
    anchorX,
    anchorY,
    `<g transform="translate(${anchorX} ${anchorY})">\n${body}\n</g>`,
    { "data-walk-cycle-length": formatNumber(person.stride * 4), "data-description": person.description },
    theme,
  );
}
