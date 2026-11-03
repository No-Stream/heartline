// The original look: candy colours, deep plum ink, curly lollipop trees and chibi people.
import { INK, shadePlum } from "../ink.ts";
import { COLORS } from "../palette.ts";
import type { Theme } from "../theme.ts";

export const CANDY: Theme = {
  name: "candy",
  description: "Candy colours, plum ink, curly lollipop trees and chibi people",
  color: (designColor) => designColor,
  shade: shadePlum,
  line: { color: INK, weight: 1, wobble: 1, overshoot: 1, breaks: 0 },
  shapeWobble: 1,
  shadow: { color: "#4b2a7a", opacity: 1, roundShadeOpacity: 0.16 },
  fill: "solid",
  paper: COLORS.paper,
  misregistration: [0, 0],
  hatching: 1,
  crookedness: 1,
  highlight: { color: "#ffffff", opacity: 1 },
  props: { sunFace: true, binFace: true, coneStripes: true, scallops: true, curls: true, mushrooms: true, clouds: "puffs", bushes: "puffs", ornaments: "candy" },
  tileGridOpacity: 0.14,
  people: {
    headScale: 1,
    childHeadScale: 1,
    heightScale: 1,
    childHeightScale: 1,
    hipFraction: 0.44,
    limbScale: 1,
    outline: 1,
    face: "candy",
  },
  trees: [
    { id: "tree_lollipop_pink", description: "Pink curly lollipop tree", height: 112, shape: { kind: "lollipop", crown: COLORS.leafPink, curl: "#d94a78", crownRadius: 15, trunkHeight: 26, direction: 1 } },
    { id: "tree_lollipop_yellow", description: "Yellow curly lollipop tree", height: 112, shape: { kind: "lollipop", crown: COLORS.leafYellow, curl: "#c97f15", crownRadius: 17, trunkHeight: 28, direction: -1 } },
    { id: "tree_lollipop_green", description: "Small green curly lollipop tree", height: 92, shape: { kind: "lollipop", crown: COLORS.leafGreen, curl: "#23865a", crownRadius: 12.5, trunkHeight: 18, direction: 1 } },
    { id: "tree_lollipop_teal", description: "Tall teal curly lollipop tree", height: 118, shape: { kind: "lollipop", crown: COLORS.leafTeal, curl: "#16705f", crownRadius: 14, trunkHeight: 40, direction: -1 } },
    { id: "tree_cloud", description: "Puffy cloud tree with curled leaves", height: 96, shape: { kind: "cloud", color: COLORS.leafGreen, accent: "#23865a" } },
    { id: "tree_cloud_blossom", description: "Puffy pink blossom tree", height: 96, shape: { kind: "cloud", color: COLORS.leafPink, accent: "#d94a78" } },
    { id: "tree_pine", description: "Three-tier swirly pine with a yellow tip", height: 102, shape: { kind: "pine", color: COLORS.leafTeal } },
  ],
  defs: "",
  spriteFilter: "",
  scene: { sky: ["#59c8e6", "#a7e6dd", "#ffe6a8"], grain: 1, background: "#fff8e8" },
};
