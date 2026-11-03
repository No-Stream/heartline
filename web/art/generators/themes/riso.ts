// A risograph print: four muted inks on warm paper. Every colour is one ink at some tint, or two
// inks printed over each other, so the whole park shares one small set of colours. Colour plates
// sit slightly out of register with the line plate, and the ink is speckled where it ran thin.
import { mix, multiply, nearestIn, paletteMap } from "../color.ts";
import { INK } from "../ink.ts";
import { COLORS } from "../palette.ts";
import type { Theme } from "../theme.ts";

const PAPER = "#f3ecdd";
const TEAL = "#1d7874";
const WARM_RED = "#dc5a3f";
const MUSTARD = "#e2a72e";
const PLUM = "#3b2741";

/** One ink laid down as a halftone tint over paper. */
const tint = (ink: string, amount: number): string => mix(PAPER, ink, amount);
/** Two tints printed over each other. */
const over = (first: string, second: string): string => multiply(first, second);

const TINTS = [0.18, 0.35, 0.55, 0.75, 1];
const INKS = [TEAL, WARM_RED, MUSTARD, PLUM];
const PRINTABLE = [
  PAPER,
  ...INKS.flatMap((ink) => TINTS.map((amount) => tint(ink, amount))),
  ...INKS.flatMap((first, firstIndex) =>
    INKS.slice(firstIndex + 1).flatMap((second) =>
      [0.35, 0.7].flatMap((firstAmount) => [0.35, 0.7].map((secondAmount) => over(tint(first, firstAmount), tint(second, secondAmount)))),
    ),
  ),
];

const GRASS = over(tint(MUSTARD, 0.5), tint(TEAL, 0.32));
const LEAF = over(tint(MUSTARD, 0.5), tint(TEAL, 0.8));

const OVERRIDES: Record<string, string> = {
  [INK]: PLUM,
  [COLORS.grass]: GRASS,
  [COLORS.grassAlternate]: over(tint(MUSTARD, 0.54), tint(TEAL, 0.35)),
  [COLORS.path]: tint(MUSTARD, 0.12),
  [COLORS.earth]: over(tint(WARM_RED, 0.6), tint(MUSTARD, 0.45)),
  [COLORS.water]: tint(TEAL, 0.55),
  [COLORS.sky]: tint(TEAL, 0.3),
  [COLORS.trunk]: tint(PLUM, 0.8),
  [COLORS.leafGreen]: LEAF,
  [COLORS.leafTeal]: TEAL,
  [COLORS.leafYellow]: MUSTARD,
  [COLORS.leafPink]: tint(WARM_RED, 0.55),
  [COLORS.cream]: tint(MUSTARD, 0.2),
  [COLORS.paper]: PAPER,
  "#ffffff": PAPER,
  "#fff8ee": PAPER,
  [COLORS.red]: WARM_RED,
  [COLORS.blue]: tint(TEAL, 0.85),
  [COLORS.yellow]: MUSTARD,
  [COLORS.green]: over(tint(TEAL, 0.7), tint(MUSTARD, 0.35)),
  [COLORS.purple]: over(tint(PLUM, 0.45), tint(WARM_RED, 0.3)),
  [COLORS.orange]: over(tint(WARM_RED, 0.75), tint(MUSTARD, 0.6)),
  [COLORS.violet]: over(tint(PLUM, 0.6), tint(TEAL, 0.45)),
  [COLORS.pink]: tint(WARM_RED, 0.45),
  [COLORS.mint]: tint(TEAL, 0.35),
  [COLORS.peach]: over(tint(MUSTARD, 0.45), tint(WARM_RED, 0.22)),
  [COLORS.plumDark]: PLUM,
  [COLORS.wood]: over(tint(MUSTARD, 0.7), tint(WARM_RED, 0.4)),
  [COLORS.woodDark]: over(tint(PLUM, 0.6), tint(WARM_RED, 0.4)),
};

export const RISO: Theme = {
  name: "riso",
  description: "Risograph print in teal, warm red, mustard and plum, with grain and slightly misregistered plates",
  color: paletteMap(OVERRIDES, PRINTABLE, (designColor) => nearestIn(PRINTABLE, designColor, 0.6)),
  // A shaded side is the same colour with a plum tint printed over it.
  shade: (color, amount) => multiply(color, tint(PLUM, Math.min(0.6, amount * 1.25))),
  line: { color: PLUM, weight: 0.78, wobble: 0.7, overshoot: 0.5, breaks: 0 },
  shapeWobble: 0.55,
  shadow: { color: PLUM, opacity: 0.85, roundShadeOpacity: 0.2 },
  fill: "solid",
  paper: PAPER,
  misregistration: [0.75, -0.5],
  hatching: 0,
  crookedness: 0.35,
  highlight: { color: PAPER, opacity: 0.55 },
  props: { sunFace: false, binFace: false, coneStripes: false, scallops: false, curls: false, mushrooms: false, clouds: "puffs", bushes: "puffs", ornaments: "candy" },
  tileGridOpacity: 0.06,
  people: {
    headScale: 0.56,
    childHeadScale: 0.62,
    heightScale: 1.08,
    childHeightScale: 1.04,
    hipFraction: 0.48,
    limbScale: 0.82,
    outline: 0.7,
    face: "dots",
  },
  trees: [
    { id: "tree_round_teal", description: "Round tree printed in teal and mustard, green where they overlap", height: 112, shape: { kind: "printedBlob", first: COLORS.water, second: COLORS.leafYellow, crownRadius: 16, trunkHeight: 24, lobes: 5 } },
    { id: "tree_round_autumn", description: "Round autumn tree printed in warm red and mustard", height: 112, shape: { kind: "printedBlob", first: COLORS.red, second: COLORS.leafYellow, crownRadius: 17, trunkHeight: 26, lobes: 6 } },
    { id: "tree_round_small", description: "Small round tree in teal and mustard", height: 92, shape: { kind: "printedBlob", first: COLORS.leafYellow, second: COLORS.water, crownRadius: 12, trunkHeight: 18, lobes: 4 } },
    { id: "tree_poplar", description: "Tall narrow poplar", height: 122, shape: { kind: "poplar", color: COLORS.leafTeal, overprint: COLORS.leafYellow, height: 74, width: 11 } },
    { id: "tree_round_wide", description: "Wide many-lobed tree in teal over mustard", height: 100, shape: { kind: "printedBlob", first: COLORS.leafGreen, second: COLORS.water, crownRadius: 19, trunkHeight: 16, lobes: 7 } },
    { id: "tree_blossom", description: "Blossom tree in a warm red tint", height: 100, shape: { kind: "printedBlob", first: COLORS.leafPink, second: COLORS.pink, crownRadius: 15, trunkHeight: 20, lobes: 6 } },
    { id: "tree_poplar_dark", description: "Dark narrow poplar in teal over plum", height: 116, shape: { kind: "poplar", color: COLORS.leafTeal, overprint: COLORS.violet, height: 66, width: 10 } },
  ],
  defs:
    // Thin spots in the ink, where the paper shows through as tiny specks, and slightly rough edges.
    `<filter id="texture" x="-4%" y="-4%" width="108%" height="108%" color-interpolation-filters="sRGB">` +
    `<feTurbulence type="fractalNoise" baseFrequency="1.1" numOctaves="2" seed="11" result="grain"/>` +
    `<feDisplacementMap in="SourceGraphic" in2="grain" scale="0.7" xChannelSelector="R" yChannelSelector="G" result="rough"/>` +
    `<feColorMatrix in="grain" type="matrix" values="0 0 0 0 0.953  0 0 0 0 0.925  0 0 0 0 0.867  0 0 0 -36 9.8" result="specks"/>` +
    `<feComposite in="specks" in2="rough" operator="in" result="specksOnInk"/>` +
    `<feMerge><feMergeNode in="rough"/><feMergeNode in="specksOnInk"/></feMerge>` +
    `</filter>`,
  spriteFilter: "url(#texture)",
  scene: { sky: [tint(TEAL, 0.32), tint(TEAL, 0.12), tint(MUSTARD, 0.2)], grain: 1.6, background: PAPER },
};
