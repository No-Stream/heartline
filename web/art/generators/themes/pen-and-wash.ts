// Loose pen line and sparse watercolour, in the spirit of Sempé and Quentin Blake. Shapes are
// mostly paper; washes cover part of each shape and run a little past the line. People are tall
// and thin with small heads.
import { mix, multiply, paletteMap, regrade } from "../color.ts";
import { INK } from "../ink.ts";
import { COLORS } from "../palette.ts";
import type { Theme } from "../theme.ts";

const PAPER = "#f8f4ea";
const PEN = "#2e2a27";
const GLAZE = "#8d96b6";

const OVERRIDES: Record<string, string> = {
  [INK]: PEN,
  [COLORS.grass]: "#e7e8cf",
  [COLORS.grassAlternate]: "#e3e5ca",
  [COLORS.path]: "#f3eee0",
  [COLORS.peach]: "#ecd2b4",
  [COLORS.earth]: "#d8b48e",
  [COLORS.water]: "#9fc6d2",
  [COLORS.sky]: "#bcd6e0",
  [COLORS.trunk]: "#6b5546",
  [COLORS.leafGreen]: "#97b07a",
  [COLORS.leafTeal]: "#7ea698",
  [COLORS.leafYellow]: "#dcc06c",
  [COLORS.leafPink]: "#e5a8a0",
  [COLORS.cream]: "#f1e6cf",
  [COLORS.paper]: PAPER,
  "#ffffff": PAPER,
  "#fff8ee": PAPER,
  [COLORS.red]: "#d66a52",
  [COLORS.blue]: "#7097bc",
  [COLORS.yellow]: "#e9c55e",
  [COLORS.plumDark]: "#4a4150",
  [COLORS.violet]: "#8f8fbf",
  [COLORS.purple]: "#b49ac2",
  [COLORS.pink]: "#e6aaa6",
  [COLORS.mint]: "#b5d3c4",
  [COLORS.green]: "#8fb391",
  [COLORS.orange]: "#e3a072",
  [COLORS.wood]: "#c9a27a",
  [COLORS.woodDark]: "#8b6a54",
};

export const PEN_AND_WASH: Theme = {
  name: "pen-and-wash",
  description: "Loose pen line and sparse watercolour washes, with tall, wry people",
  color: paletteMap(OVERRIDES, [PAPER, PEN], (designColor) => regrade(designColor, { chroma: 0.5, lightnessTarget: 0.86, lightnessPull: 0.35 })),
  // Shaded sides get a second, cool glaze over the first.
  shade: (color, amount) => multiply(color, mix("#ffffff", GLAZE, Math.min(0.75, amount * 1.7))),
  line: { color: PEN, weight: 0.62, wobble: 1.8, overshoot: 2.6, breaks: 0.3 },
  shapeWobble: 1.3,
  shadow: { color: "#5d6688", opacity: 0.55, roundShadeOpacity: 0.12 },
  fill: "wash",
  paper: PAPER,
  misregistration: [0, 0],
  hatching: 1.3,
  crookedness: 0.9,
  highlight: { color: PAPER, opacity: 0 },
  props: { sunFace: false, binFace: false, coneStripes: false, scallops: false, curls: false, mushrooms: false, clouds: "puffs", bushes: "puffs", ornaments: "candy" },
  tileGridOpacity: 0,
  people: {
    headScale: 0.5,
    childHeadScale: 0.6,
    heightScale: 1.14,
    childHeightScale: 1.05,
    hipFraction: 0.53,
    limbScale: 0.55,
    outline: 0.55,
    face: "dots",
  },
  trees: [
    { id: "tree_scribble_round", description: "Round tree drawn with a loose pen and a green wash", height: 112, shape: { kind: "scribble", color: COLORS.leafGreen, crownRadius: 17, trunkHeight: 24, lean: 2 } },
    { id: "tree_scribble_ochre", description: "Round tree with an ochre wash", height: 112, shape: { kind: "scribble", color: COLORS.leafYellow, crownRadius: 16, trunkHeight: 26, lean: -2 } },
    { id: "tree_scribble_small", description: "Small sketched tree", height: 92, shape: { kind: "scribble", color: COLORS.leafGreen, crownRadius: 12, trunkHeight: 18, lean: 1 } },
    { id: "tree_willowy", description: "Tall thin poplar in pen", height: 122, shape: { kind: "willowy", color: COLORS.leafTeal, height: 76 } },
    { id: "tree_scribble_wide", description: "Wide leaning tree with a blue-green wash", height: 104, shape: { kind: "scribble", color: COLORS.leafTeal, crownRadius: 20, trunkHeight: 18, lean: -3 } },
    { id: "tree_scribble_blossom", description: "Blossom tree with a pink wash", height: 100, shape: { kind: "scribble", color: COLORS.leafPink, crownRadius: 15, trunkHeight: 20, lean: 2 } },
    { id: "tree_willowy_short", description: "Short poplar in pen", height: 108, shape: { kind: "willowy", color: COLORS.leafGreen, height: 60 } },
  ],
  defs:
    // A wash that bleeds a little, covers the shape in patches and lets the paper show through.
    `<filter id="wash" x="-15%" y="-15%" width="130%" height="130%" color-interpolation-filters="sRGB">` +
    `<feTurbulence type="fractalNoise" baseFrequency="0.09" numOctaves="2" seed="4" result="flow"/>` +
    `<feDisplacementMap in="SourceGraphic" in2="flow" scale="3" xChannelSelector="R" yChannelSelector="G" result="bled"/>` +
    `<feTurbulence type="fractalNoise" baseFrequency="0.014" numOctaves="2" seed="9" result="coverage"/>` +
    `<feColorMatrix in="coverage" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 5 -1.9" result="patches"/>` +
    `<feComposite in="bled" in2="patches" operator="in" result="patchy"/>` +
    `<feComponentTransfer in="patchy" result="glaze"><feFuncA type="table" tableValues="0 0.8"/></feComponentTransfer>` +
    // Pigment gathers where the wash dries, leaving a slightly darker rim.
    `<feMorphology in="glaze" operator="erode" radius="0.9" result="inner"/>` +
    `<feComposite in="glaze" in2="inner" operator="out" result="rim"/>` +
    `<feColorMatrix in="rim" type="matrix" values="0.72 0 0 0 0  0 0.72 0 0 0  0 0 0.75 0 0  0 0 0 0.55 0" result="darkRim"/>` +
    `<feMerge><feMergeNode in="glaze"/><feMergeNode in="darkRim"/></feMerge>` +
    `</filter>`,
  spriteFilter: "",
  scene: { sky: ["#cfdfe6", "#eef0ea", "#f8f1e2"], grain: 2.4, background: PAPER },
};
