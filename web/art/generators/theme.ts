// A theme is the whole visual style of the drawings: palette, line, people proportions, tree and
// prop shapes, and print texture. The generators draw the same park in any theme.
//
// Colours inside the generators and in the cast are "design colours" from the original candy
// palette. A theme maps each design colour into its own palette exactly once, when it is drawn,
// so the drawings never need a per-theme colour list.

export type ThemeName = "candy" | "riso" | "pen-and-wash" | "mid-century" | "print-modern";

export interface LineStyle {
  /** Ink colour of all outlines and line work. */
  color: string;
  /** Multiplies every line width. */
  weight: number;
  /** Multiplies the unsteadiness of a line along its length. */
  wobble: number;
  /** Multiplies how far corner strokes run past the corner. */
  overshoot: number;
  /** Chance from 0 to 1 that a long outline stroke is lifted part way, as a quick pen does. */
  breaks: number;
}

export interface PeopleStyle {
  /** Multiplies the head radius of adults, so 1 keeps the chibi head of about a third of the height. */
  headScale: number;
  childHeadScale: number;
  /** Multiplies the height of adults and children. */
  heightScale: number;
  childHeightScale: number;
  /** Hip height as a fraction of full height; more is longer legs. */
  hipFraction: number;
  /** Multiplies limb thickness. */
  limbScale: number;
  /** Multiplies the outline width of people. */
  outline: number;
  face: "candy" | "dots" | "none";
}

/**
 * Optional print finish for the geometric trees: a second ink printed over the lit side, so the
 * overlap makes a third colour, and a hand-inked key line round the silhouette and trunk.
 */
export interface PrintFinish {
  overprint?: string;
  keyLine?: boolean;
}

/** Tree drawings by kind. Colours are design colours. */
export type TreeShape =
  | { kind: "lollipop"; crown: string; curl: string; crownRadius: number; trunkHeight: number; direction: number }
  | { kind: "cloud"; color: string; accent: string }
  | { kind: "pine"; color: string }
  | { kind: "printedBlob"; first: string; second: string; crownRadius: number; trunkHeight: number; lobes: number }
  | { kind: "poplar"; color: string; overprint: string; height: number; width: number }
  | { kind: "scribble"; color: string; crownRadius: number; trunkHeight: number; lean: number }
  | { kind: "willowy"; color: string; height: number }
  | ({ kind: "cone"; color: string; height: number; radius: number } & PrintFinish)
  | ({ kind: "oval"; color: string; height: number; radius: number; trunkHeight: number } & PrintFinish)
  | ({ kind: "tiers"; color: string; tiers: number; radius: number; trunkHeight: number } & PrintFinish)
  | ({ kind: "cypress"; color: string; height: number; radius: number } & PrintFinish);

export interface TreeDesign {
  id: string;
  description: string;
  height: number;
  shape: TreeShape;
}

export interface Theme {
  name: ThemeName;
  description: string;
  /** Maps a design colour into this theme's palette. */
  color: (designColor: string) => string;
  /** Darkens a theme colour for a surface turned away from the light, by an amount up to about 0.34. */
  shade: (themeColor: string, amount: number) => string;
  line: LineStyle;
  /** Multiplies the wobble of filled shapes; 0 gives crisp geometry. */
  shapeWobble: number;
  /** Cast shadows on the ground, and the soft shade on the dark side of round things. */
  shadow: { color: string; opacity: number; roundShadeOpacity: number };
  /**
   * "solid" fills each shape with flat colour. "wash" lays paper first and then a watercolour wash
   * that leaves some of the paper showing, so shapes still hide what is behind them.
   */
  fill: "solid" | "wash";
  paper: string;
  /** Offset of colour fills against the line work, in sprite pixels, as when print plates are out of register. */
  misregistration: readonly [number, number];
  /** Multiplies the number of hatching lines on shaded walls. */
  hatching: number;
  /** Multiplies the lean, taper and bend of buildings. */
  crookedness: number;
  /** Glossy highlights on balls, balloons and tree crowns; opacity multiplies each drawing's own. */
  highlight: { color: string; opacity: number };
  props: {
    sunFace: boolean;
    binFace: boolean;
    /** Alternating stripes on cone roofs, as on a circus tent. */
    coneStripes: boolean;
    /** Scalloped hems on awnings and cone roofs. */
    scallops: boolean;
    /** Swirls in bushes, pines and cloud trees. */
    curls: boolean;
    /** Spotted mushroom stools; without them the park has small planters. */
    mushrooms: boolean;
    /** Round puffy clouds, or long flat streaks. */
    clouds: "puffs" | "streaks";
    /** Bushes made of round puffs, or smooth domes. */
    bushes: "puffs" | "domes";
  };
  /** Opacity of the faint line along the back edges of each grass tile, which shows the tile grid. */
  tileGridOpacity: number;
  people: PeopleStyle;
  trees: TreeDesign[];
  /** SVG definitions placed in every drawing, such as the texture and wash filters. */
  defs: string;
  /** Filter applied to each whole drawing, or "" for none. */
  spriteFilter: string;
  /** Settings for the demo scene. */
  scene: { sky: readonly [string, string, string]; grain: number; background: string };
}
