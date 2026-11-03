// Guests are posed as small 3D puppets and projected, so that all four walking directions
// and every walk frame come from one body. Outlines are drawn as a union: all the shapes
// of a group in ink first, then all the shapes again in colour.
// Colours in the cast are design colours; the theme maps them, and sets the body proportions.
import {
  fillPath,
  formatNumber,
  hullOfCircles,
  INK,
  type Point,
  seededRandom,
  shadePlum,
  smoothPath,
  subdivide,
  svgDocument,
  wobble,
} from "./ink.ts";
import { depthOf, project, type Vector3 } from "./iso.ts";
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

type HairStyle =
  | "afro"
  | "shortCrop"
  | "bun"
  | "bald"
  | "braid"
  | "bowl"
  | "pigtails"
  | "puffs"
  | "hijab"
  | "wavyBob"
  | "swoop";
type HatStyle = "none" | "cap" | "sunhat" | "beanie";
type LowerBody = "trousers" | "shorts" | "skirt" | "dress" | "leggings";
type Accessory = "none" | "balloon" | "backpack" | "handbag" | "camera" | "iceCream" | "satchel";

export interface PersonDesign {
  id: string;
  description: string;
  isChild: boolean;
  height: number;
  headRadius: number;
  shoulderHalfWidth: number;
  hipHalfWidth: number;
  belly: number;
  chest: number;
  limbRadius: number;
  stride: number;
  skin: string;
  hair: string;
  hairStyle: HairStyle;
  hat: HatStyle;
  hatColor: string;
  top: string;
  topStripe?: string;
  sleeves: "short" | "long";
  lower: LowerBody;
  lowerColor: string;
  shoes: string;
  accessory: Accessory;
  accessoryColor: string;
  beard?: string;
  moustache?: string;
  glasses?: string;
  overallStraps?: boolean;
}

const adult = {
  isChild: false,
  height: 43,
  headRadius: 5.9,
  shoulderHalfWidth: 4.8,
  hipHalfWidth: 3.2,
  belly: 0,
  chest: 0,
  limbRadius: 1.75,
  stride: 6,
};
const child = {
  isChild: true,
  height: 29,
  headRadius: 5.9,
  shoulderHalfWidth: 3.3,
  hipHalfWidth: 2.4,
  belly: 0.4,
  chest: 0,
  limbRadius: 1.5,
  stride: 4.2,
};
const noExtras = { hat: "none", hatColor: "#000000", accessory: "none", accessoryColor: "#000000" } as const;

export const CAST: PersonDesign[] = [
  {
    ...adult, ...noExtras, id: "maya", description: "Woman with a big afro, sunshine dress and red handbag",
    skin: "#8f5a3c", hair: "#2f1b2e", hairStyle: "afro", top: "#ffc531", sleeves: "short",
    lower: "dress", lowerColor: "#ffc531", shoes: "#ff5d5d", accessory: "handbag", accessoryColor: "#ff4d6d",
  },
  {
    ...adult, ...noExtras, id: "otis", description: "Very tall thin man with ginger hair, glasses and a striped shirt",
    height: 49, shoulderHalfWidth: 4.5, hipHalfWidth: 2.8, limbRadius: 1.5, stride: 7, headRadius: 5.7,
    skin: "#ffd9b0", hair: "#e0702c", hairStyle: "swoop", top: "#fffaf0", topStripe: "#ff5d5d", sleeves: "long",
    lower: "trousers", lowerColor: "#3f5f9f", shoes: "#6d4632", glasses: INK,
  },
  {
    ...adult, ...noExtras, id: "priya", description: "Woman with a long black braid, teal top, magenta leggings and a satchel",
    height: 40, shoulderHalfWidth: 4.2, skin: "#c98f62", hair: "#231433", hairStyle: "braid", top: "#39b6a0",
    sleeves: "long", lower: "leggings", lowerColor: "#d6336c", shoes: "#ffc531", accessory: "satchel", accessoryColor: "#b86bff",
  },
  {
    ...adult, ...noExtras, id: "gus", description: "Round bald man with a walrus moustache, loud orange shirt and shorts",
    height: 41, shoulderHalfWidth: 6, hipHalfWidth: 4.6, belly: 3.8, chest: 1, limbRadius: 2.1, headRadius: 6.2,
    skin: "#ffcfa8", hair: "#a58b76", hairStyle: "bald", moustache: "#a58b76", top: "#ff8a3c", sleeves: "short",
    lower: "shorts", lowerColor: "#39b6a0", shoes: "#f6efe1", accessory: "camera", accessoryColor: "#2a1b42",
  },
  {
    ...adult, ...noExtras, id: "lena", description: "Elderly woman with a white bun, lavender cardigan and a long skirt",
    height: 38, shoulderHalfWidth: 4.4, hipHalfWidth: 3.6, belly: 1.2, headRadius: 5.7, stride: 4.5,
    skin: "#eab98f", hair: "#f6f1ea", hairStyle: "bun", top: "#b86bff", sleeves: "long",
    lower: "skirt", lowerColor: "#3f8cff", shoes: "#6d4632", accessory: "handbag", accessoryColor: "#2ec98a", glasses: INK,
  },
  {
    ...adult, ...noExtras, id: "noor", description: "Young woman in a pink hijab and long mint coat",
    height: 42, shoulderHalfWidth: 4.3, skin: "#d9a273", hair: "#ff7fa8", hairStyle: "hijab", top: "#7fe0b8", sleeves: "long",
    lower: "skirt", lowerColor: "#7fe0b8", shoes: "#2a1b42",
  },
  {
    ...adult, ...noExtras, id: "sam", description: "Teenager with a black bowl cut, purple hoodie and jeans",
    height: 40, shoulderHalfWidth: 4.5, hipHalfWidth: 3, skin: "#f6d3a8", hair: "#1f1630", hairStyle: "bowl", top: "#6a5cff", sleeves: "long",
    lower: "trousers", lowerColor: "#3f8cff", shoes: "#ff5d5d", accessory: "iceCream", accessoryColor: "#ff8fa8",
  },
  {
    ...adult, ...noExtras, id: "hank", description: "Big dark-skinned man with a grey beard, straw sunhat and blue shirt",
    height: 45, shoulderHalfWidth: 6.2, hipHalfWidth: 4.4, belly: 2.2, chest: 1.4, limbRadius: 2.2, headRadius: 6.1,
    skin: "#6b4029", hair: "#cfc8c0", hairStyle: "shortCrop", beard: "#cfc8c0", hat: "sunhat", hatColor: "#ffe08a",
    top: "#3f8cff", sleeves: "short", lower: "trousers", lowerColor: "#d9b58a", shoes: "#6d4632",
  },
  {
    ...child, ...noExtras, id: "theo", description: "Boy with a backwards green cap, striped tee, shorts and a big backpack",
    skin: "#7c5238", hair: "#231433", hairStyle: "shortCrop", hat: "cap", hatColor: "#2ec98a", top: "#ffc531", topStripe: "#3f8cff", sleeves: "short",
    lower: "shorts", lowerColor: "#ff5d5d", shoes: "#fffaf0", accessory: "backpack", accessoryColor: "#ff8a3c",
  },
  {
    ...child, ...noExtras, id: "bea", description: "Little girl with orange pigtails, a red dress and a balloon",
    height: 27, skin: "#ffe0c4", hair: "#f08a3c", hairStyle: "pigtails", top: "#ff5d5d", sleeves: "short",
    lower: "dress", lowerColor: "#ff5d5d", shoes: "#2a1b42", accessory: "balloon", accessoryColor: "#3f8cff",
  },
  {
    ...child, ...noExtras, id: "ines", description: "Small child with two round puffs and yellow overalls over a blue tee",
    height: 25, headRadius: 5.6, stride: 3.6, skin: "#a86b46", hair: "#2a1820", hairStyle: "puffs", top: "#57cfe6", sleeves: "short",
    lower: "trousers", lowerColor: "#ffc531", shoes: "#ff5d5d", overallStraps: true,
  },
  {
    ...child, ...noExtras, id: "kai", description: "Older boy with a blue beanie, red hoodie and an ice cream",
    height: 33, headRadius: 6, shoulderHalfWidth: 3.6, stride: 4.8, skin: "#f0c090", hair: "#6d4632", hairStyle: "shortCrop", hat: "beanie", hatColor: "#3f8cff",
    top: "#ff4d6d", sleeves: "long", lower: "trousers", lowerColor: "#39445c", shoes: "#2ec98a", accessory: "iceCream", accessoryColor: "#fff2c2",
  },
];

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
  const frameCount = isWalking ? WALK_FRAME_COUNT : IDLE_FRAME_COUNT;
  const phase = (frame / frameCount) * Math.PI * 2;
  const leftIsNear = depthOf(toView(0, 1, 0)) > 0;
  const facingViewer = depthOf(toView(1, 0, 0)) > 0;

  const sizeRatio = person.height / 43;
  const hipHeight = person.height * theme.people.hipFraction;
  const legLength = hipHeight - 1.4;
  const headCenterHeight = person.height - person.headRadius;
  const shoulderHeight = headCenterHeight - person.headRadius - 0.6;
  const bounce = isWalking ? -0.55 * sizeRatio * Math.cos(phase * 2) : 0.22 * Math.sin(phase);
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
  const skirted = person.lower === "skirt";
  const legColor =
    person.lower === "shorts" || person.lower === "dress" ? person.skin : person.lowerColor;
  for (const side of [1, -1]) {
    const legPhase = phase + (side === 1 ? 0 : Math.PI);
    const footForward = isWalking ? person.stride * Math.cos(legPhase) : 0;
    const footLift = isWalking ? 3.4 * sizeRatio * Math.max(0, -Math.sin(legPhase)) ** 1.3 : 0;
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
    } else {
      addLayer(order, [capsule(hip, knee), capsule(knee, ankle)], legColor);
    }
    addLayer(order + 0.2, [[heel, toe]], person.shoes);
  }

  // Skirt and dress hems sway a little with the stride.
  const torsoLean = (height: number): number => lean * ((height - hipHeight) / (headCenterHeight - hipHeight));
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

  // Torso: shoulders and hips joined by one hull, with a belly and chest pushing it forward.
  const shoulderZ = shoulderHeight + bounce;
  const breath = isWalking ? 0 : 0.2 * Math.sin(phase);
  const leftShoulder = ball(torsoLean(shoulderHeight), person.shoulderHalfWidth * 0.72, shoulderZ + breath, 1.9 * sizeRatio ** 0.5);
  const rightShoulder = ball(torsoLean(shoulderHeight), -person.shoulderHalfWidth * 0.72, shoulderZ + breath, 1.9 * sizeRatio ** 0.5);
  const hipCore = hipHeight + bounce + 1.5;
  const torsoBalls: Ball[] = [
    leftShoulder,
    rightShoulder,
    ball(person.chest, 0, shoulderZ - 2.5, person.shoulderHalfWidth * 0.8),
    ball(person.belly, 0, hipCore + 3, person.hipHalfWidth + person.belly * 0.55),
    ball(0, person.hipHalfWidth * 0.5, hipCore, 2.4),
    ball(0, -person.hipHalfWidth * 0.5, hipCore, 2.4),
  ];
  const needsLowerTorso = person.lower === "trousers" || person.lower === "shorts" || person.lower === "leggings";
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

  // Arms swing against the legs on the same side.
  let carryingHand: Ball | undefined;
  for (const side of [1, -1]) {
    const armPhase = phase + (side === 1 ? Math.PI : 0);
    const swing = isWalking ? person.stride * 0.72 * Math.cos(armPhase) : 0;
    const isNear = (side === 1) === leftIsNear;
    const shoulderLeft = side * (person.shoulderHalfWidth * 0.86 + person.belly * 0.12);
    const shoulder = ball(torsoLean(shoulderHeight), shoulderLeft, shoulderZ + breath - 0.3, person.limbRadius * 0.95);
    const armLength = (shoulderHeight - hipHeight) * 1.0;
    const carriesAccessory = side === -1 && (person.accessory === "iceCream" || person.accessory === "balloon");
    const handForward = carriesAccessory ? 3.2 * sizeRatio ** 0.5 : torsoLean(shoulderHeight) + swing * 0.95 + person.belly * 0.5;
    const handUp = carriesAccessory
      ? shoulderZ - armLength * 0.74
      : shoulderZ - armLength + Math.abs(swing) * 0.22 + 0.5;
    const handOut = shoulderLeft + side * ((carriesAccessory ? 3.2 : 1.5) + person.belly * 0.4);
    const elbow = ball(
      (torsoLean(shoulderHeight) + handForward) * 0.42 - 0.3,
      (shoulderLeft + handOut) * 0.5 + side * 0.5,
      (shoulderZ + handUp) * 0.5,
      person.limbRadius * 0.9,
    );
    const hand = ball(handForward, handOut, handUp, person.limbRadius * 0.92);
    const order = isNear ? 6 : 0.5;
    const sleeveColor = person.sleeves === "long" ? person.top : person.skin;
    if (person.sleeves === "short") {
      addLayer(order, [capsule(shoulder, elbow), capsule(elbow, hand)], person.skin);
      const sleeveEnd: Ball = { center: lerpVector(shoulder.center, elbow.center, 0.55), radius: person.limbRadius * 1.05 };
      addLayer(order + 0.1, [capsule(shoulder, sleeveEnd)], person.top);
    } else {
      addLayer(order, [capsule(shoulder, elbow), capsule(elbow, hand)], sleeveColor);
      addLayer(order + 0.1, [[{ ...hand, radius: hand.radius * 0.95 }]], person.skin);
    }
    if (carriesAccessory) carryingHand = hand;
  }

  // Head and hair.
  const headLean = lean * 1.2 + (isWalking ? Math.cos(phase * 2) * -0.05 : 0);
  const headUp = headCenterHeight + bounce * 1.1 + breath * 0.5;
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
  if (person.accessory === "camera") {
    const chestForward = person.chest + person.shoulderHalfWidth * 0.8 + 0.2 + (person.belly > 0 ? 0.4 : 0);
    if (facingViewer) {
      const lens = project(toView(chestForward + 1, 0, shoulderZ - 3.8));
      const lensMarkup = `<circle cx="${formatNumber(lens[0])}" cy="${formatNumber(lens[1])}" r="0.9" fill="${theme.color("#a9dcf2")}"/>`;
      addLayer(5.5, [[ball(chestForward, 1.1, shoulderZ - 3.6, 1.05), ball(chestForward, -1.1, shoulderZ - 3.6, 1.05)]], "#5a4a78", lensMarkup, 0.8);
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

  const face = facingViewer ? drawFace(person, theme, headBall, leftIsNear ? 1 : -1, animation === "idle" && frame === 1) : "";

  // Things drawn above everything else, for a held balloon or ice cream.
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
  // Features are cheated towards the camera, as a character artist would: at a true three-quarter
  // angle seen from above, the far eye falls off the head and the face sits too low to read.
  const yawTowardCamera = (24 * Math.PI) / 180;
  const onHead = (forward: number, left: number, up: number): Point =>
    project(
      headBall(
        forward * Math.cos(yawTowardCamera) - left * nearSide * Math.sin(yawTowardCamera),
        left * Math.cos(yawTowardCamera) + forward * nearSide * Math.sin(yawTowardCamera),
        up + 0.2,
        0,
      ).center,
    );
  const eyeUp = person.hat === "sunhat" ? 0.02 : 0.1;
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
  if (person.moustache) {
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
      return [
        { balls: [[-0.2, 0, -0.1, 1.15]], bias: 0 },
        { balls: [[0, 0, 0.5, 0.88]], bias: 0.2, alwaysOnTop: true },
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
