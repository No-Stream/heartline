// A calm, modern print: riso's small ink set and overprinting, pulled towards mid-century's dusky
// tones and clear lit and shaded planes. A quiet hand-inked key line sits over crisp geometry,
// the colour plates are a touch out of register, and the paper texture stays faint so large grass
// areas read calmly at game size.
import { multiply, mix, nearestIn, paletteMap, regrade } from "../color.ts";
import { INK } from "../ink.ts";
import { COLORS } from "../palette.ts";
import type { Theme } from "../theme.ts";

const PAPER = "#f2eadb";
const TEAL = "#227571";
const CORAL = "#dc5c3e";
const MUSTARD = "#e9ab38";
const SLATE = "#3f61a6";
const PLUM = "#3a2e58";

/** One ink laid down as a halftone tint over paper. */
const tint = (ink: string, amount: number): string => mix(PAPER, ink, amount);
/** Two tints printed over each other. */
const over = (first: string, second: string): string => multiply(first, second);

const TINTS = [0.18, 0.35, 0.55, 0.75, 1];
const INKS = [TEAL, CORAL, MUSTARD, SLATE, PLUM];
const PRINTABLE = [
  PAPER,
  ...INKS.flatMap((ink) => TINTS.map((amount) => tint(ink, amount))),
  ...INKS.flatMap((first, firstIndex) =>
    INKS.slice(firstIndex + 1).flatMap((second) =>
      [0.35, 0.6, 0.85].flatMap((firstAmount) => [0.35, 0.6, 0.85].map((secondAmount) => over(tint(first, firstAmount), tint(second, secondAmount)))),
    ),
  ),
];

const OVERRIDES: Record<string, string> = {
  [INK]: PLUM,
  [COLORS.grass]: over(tint(MUSTARD, 0.3), tint(TEAL, 0.42)),
  [COLORS.grassAlternate]: over(tint(MUSTARD, 0.32), tint(TEAL, 0.44)),
  [COLORS.path]: tint(MUSTARD, 0.16),
  [COLORS.earth]: over(tint(CORAL, 0.55), tint(PLUM, 0.22)),
  [COLORS.water]: over(tint(TEAL, 0.58), tint(SLATE, 0.1)),
  [COLORS.sky]: tint(SLATE, 0.4),
  [COLORS.trunk]: tint(PLUM, 0.75),
  [COLORS.leafGreen]: over(tint(MUSTARD, 0.5), tint(TEAL, 0.55)),
  [COLORS.leafTeal]: TEAL,
  [COLORS.leafYellow]: MUSTARD,
  [COLORS.leafPink]: tint(CORAL, 0.7),
  [COLORS.cream]: tint(MUSTARD, 0.22),
  [COLORS.paper]: PAPER,
  "#ffffff": PAPER,
  "#fff8ee": PAPER,
  [COLORS.red]: CORAL,
  [COLORS.blue]: SLATE,
  [COLORS.yellow]: MUSTARD,
  [COLORS.green]: over(tint(MUSTARD, 0.2), tint(TEAL, 0.65)),
  [COLORS.purple]: tint(PLUM, 0.62),
  [COLORS.orange]: over(tint(CORAL, 0.6), tint(MUSTARD, 0.75)),
  [COLORS.violet]: over(tint(SLATE, 1), tint(CORAL, 0.2)),
  [COLORS.pink]: tint(CORAL, 0.45),
  [COLORS.mint]: tint(TEAL, 0.4),
  [COLORS.peach]: over(tint(MUSTARD, 0.42), tint(CORAL, 0.22)),
  [COLORS.plumDark]: PLUM,
  [COLORS.wood]: over(tint(MUSTARD, 0.7), tint(CORAL, 0.4)),
  [COLORS.woodDark]: over(tint(PLUM, 0.6), tint(CORAL, 0.4)),
};

/** Everything else: calmed towards mid-century's dusky middle tones, then printed with the nearest ink mix. */
const toPrintable = (designColor: string): string =>
  nearestIn(PRINTABLE, regrade(designColor, { chroma: 0.9, lightnessTarget: 0.64, lightnessPull: 0.12 }), 0.85);

export const PRINT_MODERN: Theme = {
  name: "print-modern",
  description: "Five dusky print inks with a quiet key line, clear lit and shaded planes, geometric overprinted trees and faint grain",
  color: paletteMap(OVERRIDES, PRINTABLE, toPrintable),
  // A shaded side is the same colour with a plum tint printed over it, strong enough for a clear plane.
  shade: (color, amount) => multiply(color, tint(PLUM, Math.min(0.58, amount * 1.45))),
  line: { color: PLUM, weight: 0.6, wobble: 0.35, overshoot: 0.2, breaks: 0 },
  shapeWobble: 0.18,
  shadow: { color: PLUM, opacity: 0.8, roundShadeOpacity: 0.26 },
  fill: "solid",
  paper: PAPER,
  misregistration: [0.5, -0.35],
  hatching: 0,
  crookedness: 0.12,
  highlight: { color: PAPER, opacity: 0.3 },
  props: { sunFace: false, binFace: false, coneStripes: false, scallops: false, curls: false, mushrooms: false, clouds: "streaks", bushes: "domes", ornaments: "plain" },
  tileGridOpacity: 0.03,
  people: {
    headScale: 0.64,
    childHeadScale: 0.66,
    heightScale: 1.06,
    childHeightScale: 1.04,
    hipFraction: 0.5,
    limbScale: 0.9,
    outline: 0.55,
    face: "dots",
  },
  trees: [
    { id: "tree_cone", description: "Teal cone tree with a mustard overprint", height: 104, shape: { kind: "cone", color: COLORS.leafTeal, height: 44, radius: 15, overprint: COLORS.leafYellow, keyLine: true } },
    { id: "tree_oval", description: "Green oval tree on a thin trunk, with a mustard overprint", height: 112, shape: { kind: "oval", color: COLORS.leafGreen, height: 34, radius: 13, trunkHeight: 18, overprint: COLORS.leafYellow, keyLine: true } },
    { id: "tree_cone_ochre", description: "Small mustard cone tree with a coral overprint", height: 88, shape: { kind: "cone", color: COLORS.leafYellow, height: 30, radius: 11, overprint: COLORS.pink, keyLine: true } },
    { id: "tree_cypress", description: "Tall teal cypress with a mustard overprint", height: 124, shape: { kind: "cypress", color: COLORS.leafTeal, height: 76, radius: 8, overprint: COLORS.leafYellow, keyLine: true } },
    { id: "tree_tiers", description: "Green stone pine of three flat layers", height: 100, shape: { kind: "tiers", color: COLORS.leafGreen, tiers: 3, radius: 18, trunkHeight: 22, keyLine: true } },
    { id: "tree_oval_coral", description: "Coral oval tree with a mustard overprint", height: 104, shape: { kind: "oval", color: COLORS.leafPink, height: 28, radius: 12, trunkHeight: 16, overprint: COLORS.leafYellow, keyLine: true } },
    { id: "tree_cypress_short", description: "Short green cypress with a teal overprint", height: 106, shape: { kind: "cypress", color: COLORS.leafGreen, height: 56, radius: 8.5, overprint: COLORS.leafTeal, keyLine: true } },
  ],
  defs:
    // Riso's ink texture, much quieter: a rare paper speck where the ink ran thin and barely rough edges.
    `<filter id="texture" x="-4%" y="-4%" width="108%" height="108%" color-interpolation-filters="sRGB">` +
    `<feTurbulence type="fractalNoise" baseFrequency="1.1" numOctaves="2" seed="11" result="grain"/>` +
    `<feDisplacementMap in="SourceGraphic" in2="grain" scale="0.35" xChannelSelector="R" yChannelSelector="G" result="rough"/>` +
    `<feColorMatrix in="grain" type="matrix" values="0 0 0 0 0.949  0 0 0 0 0.918  0 0 0 0 0.859  0 0 0 -30 6.0" result="specks"/>` +
    `<feComponentTransfer in="specks" result="faintSpecks"><feFuncA type="linear" slope="0.45"/></feComponentTransfer>` +
    `<feComposite in="faintSpecks" in2="rough" operator="in" result="specksOnInk"/>` +
    `<feMerge><feMergeNode in="rough"/><feMergeNode in="specksOnInk"/></feMerge>` +
    `</filter>`,
  spriteFilter: "url(#texture)",
  scene: { sky: [tint(SLATE, 0.42), tint(TEAL, 0.14), tint(MUSTARD, 0.28)], grain: 1.1, background: PAPER },
};
