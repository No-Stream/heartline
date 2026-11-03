// Cast data and the design vocabulary shared by person renderers.
import { INK } from "./ink.ts";

export type HairStyle =
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
  | "swoop"
  | "balding"
  | "highBun";
type HatStyle = "none" | "cap" | "sunhat" | "beanie";
type LowerBody = "trousers" | "shorts" | "cargoShorts" | "skirt" | "dress" | "leggings";
type Accessory = "none" | "balloon" | "backpack" | "handbag" | "camera" | "iceCream" | "satchel" | "tote" | "walkingStick" | "drink" | "beltBag";

/** How a person walks and stands, as scales on the usual gait. */
export interface Gait {
  footLift: number;
  armSwing: number;
  bounce: number;
  /** How far the shoulders and head sit forward of the hips, in body units, walking or standing. */
  stoop: number;
}
export const USUAL_GAIT: Gait = { footLift: 1, armSwing: 1, bounce: 1, stoop: 0 };

export interface PersonDesign {
  id: string;
  description: string;
  isChild: boolean;
  ageGroup: PersonAgeGroup;
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
  /** Fine round glasses in this frame colour, drawn into the candy face; other face styles draw them with the framed lenses. */
  glasses?: string;
  /** Glasses held on by a strap round the back of the head, drawn in every face style; the strap's colour. */
  glassesStrap?: string;
  /** A roller coaster printed on the front and back of the top, in this colour. */
  coasterPrint?: string;
  socks?: string;
  pocketColor?: string;
  overallStraps?: boolean;
  /** Long sleeves in another colour from the torso, as for a sleeveless vest over a shirt. */
  sleeveColor?: string;
  /** An open cardigan or jacket: the colour of the shirt that shows down the front. */
  openFront?: string;
  /** A knee-length tunic worn over the trousers, in this colour. */
  tunic?: string;
  /** Glasses with an ink frame drawn in every face style, the candy face included, as the strapped glasses are. */
  framedGlasses?: boolean;
  gait?: Gait;
}

export type PersonAgeGroup = "child" | "adult" | "senior";
export type PersonAppearance = {
  ageGroup: PersonAgeGroup;
  skinToneGroup: "light" | "medium" | "deep";
  hairGroup: "curly" | "short" | "tied" | "covered" | "bob" | "bald";
  clothingPaletteGroup: "red" | "orange" | "yellow" | "green" | "blue" | "purple" | "neutral";
  bodyShape: "slender" | "average" | "broad";
};

const adult = {
  isChild: false,
  ageGroup: "adult" as const,
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
  ageGroup: "child" as const,
  height: 29,
  headRadius: 5.9,
  shoulderHalfWidth: 3.3,
  hipHalfWidth: 2.4,
  belly: 0.4,
  chest: 0,
  limbRadius: 1.5,
  stride: 4.2,
};
/** Older people take shorter, lower steps with less swing and bounce. */
const senior = {
  ...adult,
  ageGroup: "senior" as const,
  height: 40,
  stride: 4.6,
  gait: { footLift: 0.62, armSwing: 0.7, bounce: 0.55, stoop: 0 },
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
    ageGroup: "senior",
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
    ...adult, ...noExtras, id: "ada", description: "Tall slim young woman with a high bun of locs, a red top, a long mustard skirt and a big fizzy drink",
    height: 46, shoulderHalfWidth: 4.3, hipHalfWidth: 3, limbRadius: 1.6, headRadius: 5.8, stride: 6.6,
    skin: "#6e4330", hair: "#2a1a22", hairStyle: "highBun", top: "#e8483c", sleeves: "short",
    lower: "skirt", lowerColor: "#e9b44c", shoes: "#2a1b42", accessory: "drink", accessoryColor: "#39b6a0",
  },
  {
    ...adult, ...noExtras, id: "kofi", description: "Bald dad in his forties with a pink shirt, dark jeans, white trainers and an orange belt bag",
    height: 44, shoulderHalfWidth: 5.2, hipHalfWidth: 3.5, belly: 1.2, chest: 0.6, limbRadius: 1.9, headRadius: 6,
    skin: "#4f3122", hair: "#1f1618", hairStyle: "bald", top: "#ff7fa8", sleeves: "short",
    lower: "trousers", lowerColor: "#39445c", shoes: "#fffaf0", accessory: "beltBag", accessoryColor: "#ff8a3c",
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
  {
    ...adult, ...noExtras, id: "woody", description: "Coaster enthusiast in a roller coaster T-shirt, cargo shorts and glasses on a strap",
    height: 43, shoulderHalfWidth: 5.3, hipHalfWidth: 3.6, belly: 1.6, chest: 0.4, limbRadius: 1.85, headRadius: 6,
    skin: "#e8b48a", hair: "#5a3a28", hairStyle: "shortCrop", top: "#39445c", coasterPrint: "#ffc531", sleeves: "short",
    lower: "cargoShorts", lowerColor: "#c9a26b", pocketColor: "#b08550", socks: "#fffaf0", shoes: "#6d4632", glassesStrap: "#ff5d5d",
  },
  {
    ...senior, ...noExtras, id: "earl", description: "Broad older man with cropped silver hair and an open green cardigan",
    height: 42, shoulderHalfWidth: 6, hipHalfWidth: 4.4, belly: 2.4, chest: 1, limbRadius: 2.1, headRadius: 6,
    skin: "#5a3826", hair: "#e2dfda", hairStyle: "shortCrop",
    top: "#3f8a5c", openFront: "#fffaf0", sleeves: "long", lower: "trousers", lowerColor: "#5c5866", shoes: "#2a1b42",
    gait: { ...senior.gait, stoop: 0.5 },
  },
  {
    ...senior, ...noExtras, id: "mei", description: "Slim older woman with a silver bob, glasses, a light mustard jacket and a teal tote bag",
    height: 38, shoulderHalfWidth: 3.8, hipHalfWidth: 2.6, limbRadius: 1.55, headRadius: 5.6,
    skin: "#f0cfa8", hair: "#e4e3e6", hairStyle: "wavyBob", framedGlasses: true,
    top: "#e9b44c", openFront: "#fffaf0", sleeves: "long", lower: "trousers", lowerColor: "#39445c", shoes: "#fffaf0",
    accessory: "tote", accessoryColor: "#2e8f7a", gait: { ...senior.gait, stoop: 0.3 },
  },
  {
    ...senior, ...noExtras, id: "ravi", description: "Older man with a grey beard and a walking stick, a maroon vest over a long cream kurta",
    height: 40, shoulderHalfWidth: 4.8, hipHalfWidth: 3.4, belly: 1.4, limbRadius: 1.8, headRadius: 5.9,
    skin: "#a8714a", hair: "#d4d0ca", hairStyle: "balding", beard: "#d4d0ca",
    top: "#8a3f4a", sleeves: "long", sleeveColor: "#f3ead8", tunic: "#f3ead8", lower: "trousers", lowerColor: "#9a9488", shoes: "#6d4632",
    accessory: "walkingStick", accessoryColor: "#4a2f22", gait: { ...senior.gait, stoop: 1.1 },
  },
];
