// Dusky geometric shapes and flat colour fields, in the spirit of Monument Valley and mid-century
// illustration. Straight buildings, faces split into lit and shaded planes, stylised trees and a
// calm dusk sky.
import { mix, paletteMap, regrade } from "../color.ts";
import { INK } from "../ink.ts";
import { COLORS } from "../palette.ts";
import type { Theme } from "../theme.ts";

const DUSK_SHADE = "#3a375a";
const LINE = "#2f2b40";
const PAPER = "#f3e8d4";

const OVERRIDES: Record<string, string> = {
  [INK]: LINE,
  [COLORS.grass]: "#93a47c",
  [COLORS.grassAlternate]: "#90a279",
  [COLORS.path]: "#e6d3b1",
  [COLORS.peach]: "#dcb48c",
  [COLORS.earth]: "#b8704f",
  [COLORS.water]: "#6ea2a5",
  [COLORS.sky]: "#9fbcc4",
  [COLORS.trunk]: "#5a4850",
  [COLORS.leafGreen]: "#4f7d66",
  [COLORS.leafTeal]: "#2f6b6c",
  [COLORS.leafYellow]: "#d3a24a",
  [COLORS.leafPink]: "#d38b7c",
  [COLORS.cream]: "#eee0c4",
  [COLORS.paper]: PAPER,
  "#ffffff": PAPER,
  "#fff8ee": "#f6e9dc",
  [COLORS.red]: "#c4553e",
  [COLORS.blue]: "#3f5f8a",
  [COLORS.yellow]: "#e0ad4a",
  [COLORS.green]: "#5e8d6d",
  [COLORS.purple]: "#8b6b8e",
  [COLORS.orange]: "#d6814b",
  [COLORS.violet]: "#4d4a78",
  [COLORS.pink]: "#da988c",
  [COLORS.mint]: "#9bbfac",
  [COLORS.plumDark]: "#3a3348",
  [COLORS.wood]: "#b48655",
  [COLORS.woodDark]: "#77543f",
};

export const MID_CENTURY: Theme = {
  name: "mid-century",
  description: "Dusky flat colour, crisp geometry, lit and shaded planes and stylised trees",
  color: paletteMap(OVERRIDES, [PAPER, LINE], (designColor) => regrade(designColor, { chroma: 0.6, lightnessTarget: 0.62, lightnessPull: 0.2 })),
  shade: (color, amount) => mix(color, DUSK_SHADE, Math.min(0.62, amount * 1.9)),
  line: { color: LINE, weight: 0.3, wobble: 0.2, overshoot: 0, breaks: 0 },
  shapeWobble: 0.05,
  shadow: { color: DUSK_SHADE, opacity: 1, roundShadeOpacity: 0.3 },
  fill: "solid",
  paper: PAPER,
  misregistration: [0, 0],
  hatching: 0,
  crookedness: 0,
  highlight: { color: PAPER, opacity: 0 },
  props: { sunFace: false, binFace: false, coneStripes: false, scallops: false, curls: false, mushrooms: false, clouds: "streaks", bushes: "domes", ornaments: "candy" },
  tileGridOpacity: 0,
  people: {
    headScale: 0.55,
    childHeadScale: 0.62,
    heightScale: 1.08,
    childHeightScale: 1.04,
    hipFraction: 0.5,
    limbScale: 0.9,
    outline: 0.35,
    face: "none",
  },
  trees: [
    { id: "tree_cone", description: "Teal cone tree", height: 104, shape: { kind: "cone", color: COLORS.leafTeal, height: 44, radius: 15 } },
    { id: "tree_oval", description: "Sage oval tree on a thin trunk", height: 112, shape: { kind: "oval", color: COLORS.leafGreen, height: 34, radius: 13, trunkHeight: 18 } },
    { id: "tree_cone_ochre", description: "Small ochre cone tree", height: 88, shape: { kind: "cone", color: COLORS.leafYellow, height: 30, radius: 11 } },
    { id: "tree_cypress", description: "Tall dark cypress", height: 124, shape: { kind: "cypress", color: COLORS.leafTeal, height: 76, radius: 8 } },
    { id: "tree_tiers", description: "Stone pine of three flat layers", height: 100, shape: { kind: "tiers", color: COLORS.leafGreen, tiers: 3, radius: 18, trunkHeight: 22 } },
    { id: "tree_oval_coral", description: "Coral oval tree", height: 104, shape: { kind: "oval", color: COLORS.leafPink, height: 28, radius: 12, trunkHeight: 16 } },
    { id: "tree_cypress_short", description: "Short sage cypress", height: 106, shape: { kind: "cypress", color: COLORS.leafGreen, height: 56, radius: 8.5 } },
  ],
  defs: "",
  spriteFilter: "",
  scene: { sky: ["#6f7ea2", "#c4a6a6", "#eec49a"], grain: 1.8, background: PAPER },
};
