/** Time of day, the sun and the print-style colour grade that follows them. A phase of 0 is midnight and 0.5 is noon. */
export type Rgb = readonly [number, number, number];

/** One full day takes 24 real minutes at the simulation's 30 ticks per second. */
export const DAY_LENGTH_TICKS = 24 * 60 * 30;
/** A new park opens mid-morning. */
export const START_PHASE = 10 / 24;
export const NOON = 0.5;

export interface Lighting {
  readonly phase: number;
  /** The light falling on the park, multiplied into every colour. */
  readonly multiply: Rgb;
  /** A two-ink print of the image, from the darkest to the lightest tone, mixed in by `inkAmount`. */
  readonly shadowInk: Rgb;
  readonly highlightInk: Rgb;
  readonly inkAmount: number;
  readonly skyTop: Rgb;
  readonly skyHorizon: Rgb;
  /** How far lamps, windows and ride lights are switched on, from 0 to 1. */
  readonly lightsOn: number;
  readonly stars: number;
  /** Opacity of the soft ground shadows that buildings and rides cast. */
  readonly shadowStrength: number;
}

export interface Sun {
  /** Angle above the horizon in radians; negative at night. */
  readonly elevation: number;
  /** Unit vector in world tiles along which shadows fall. */
  readonly shadowDirection: { readonly x: number; readonly y: number };
}

export function hexColor(hex: string): Rgb {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error(`Invalid colour ${hex}`);
  return [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255];
}

function checkPhase(phase: number): void {
  if (!Number.isFinite(phase) || phase < 0 || phase >= 1) throw new RangeError(`Invalid day phase ${phase}`);
}

export function advanceDayPhase(phase: number, ticks: number, speed: number): number {
  checkPhase(phase);
  if (!Number.isFinite(ticks) || ticks < 0 || !Number.isFinite(speed) || speed < 0) throw new RangeError("Invalid day clock advance");
  const advanced = phase + ticks * speed / DAY_LENGTH_TICKS;
  return advanced - Math.floor(advanced);
}

export function formatClock(phase: number): string {
  checkPhase(phase);
  const minutes = Math.floor(phase * 24 * 60);
  return `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
}

const MAXIMUM_SUN_ELEVATION = Math.PI / 3;

export function sunAt(phase: number): Sun {
  checkPhase(phase);
  const elevation = MAXIMUM_SUN_ELEVATION * Math.sin(2 * Math.PI * (phase - 0.25));
  // The sun turns half a circle between sunrise and sunset; at noon shadows fall along +x +y, towards the default camera.
  const bearing = Math.PI * (phase - 0.25) / 0.5 - Math.PI / 4;
  return { elevation, shadowDirection: { x: Math.cos(bearing), y: Math.sin(bearing) } };
}

interface Keyframe {
  phase: number;
  multiply: string; shadowInk: string; highlightInk: string; inkAmount: number;
  skyTop: string; skyHorizon: string; lightsOn: number; stars: number; shadowStrength: number;
}

// The inks come from the print-modern theme: plum key line, slate, coral, mustard and paper.
const night: Omit<Keyframe, "phase"> = { multiply: "#4f4c8c", shadowInk: "#191433", highlightInk: "#8b85ca", inkAmount: 0.55, skyTop: "#17152f", skyHorizon: "#3d3766", lightsOn: 1, stars: 1, shadowStrength: 0 };
const day: Omit<Keyframe, "phase"> = { multiply: "#ffffff", shadowInk: "#3a2e58", highlightInk: "#f2eadb", inkAmount: 0, skyTop: "#9fbdc4", skyHorizon: "#efe3cc", lightsOn: 0, stars: 0, shadowStrength: 0.2 };
const keyframes: readonly Keyframe[] = [
  { phase: 0, ...night },
  { phase: 0.2, ...night },
  { phase: 0.26, multiply: "#d9c6d8", shadowInk: "#3a2e58", highlightInk: "#f0b9b0", inkAmount: 0.25, skyTop: "#6f7fae", skyHorizon: "#f2b9a0", lightsOn: 0.45, stars: 0.1, shadowStrength: 0.06 },
  { phase: 0.33, multiply: "#fff7ec", shadowInk: "#3a2e58", highlightInk: "#f7e2c0", inkAmount: 0.06, skyTop: "#a6c0cc", skyHorizon: "#f4dfc0", lightsOn: 0, stars: 0, shadowStrength: 0.18 },
  { phase: 0.42, ...day },
  { phase: 0.62, ...day },
  { phase: 0.7, multiply: "#fff0d6", shadowInk: "#3a2e58", highlightInk: "#f8d49c", inkAmount: 0.1, skyTop: "#9fb3c4", skyHorizon: "#f2c98a", lightsOn: 0, stars: 0, shadowStrength: 0.2 },
  { phase: 0.76, multiply: "#f8dccb", shadowInk: "#4a3563", highlightInk: "#f0a48c", inkAmount: 0.42, skyTop: "#5b5d93", skyHorizon: "#ec9a6a", lightsOn: 0.6, stars: 0, shadowStrength: 0.08 },
  { phase: 0.81, multiply: "#a493c4", shadowInk: "#2a2450", highlightInk: "#c49ac0", inkAmount: 0.45, skyTop: "#2f2d5c", skyHorizon: "#8c6a9a", lightsOn: 1, stars: 0.5, shadowStrength: 0 },
  { phase: 0.87, ...night },
  { phase: 1, ...night },
];

const mixNumber = (from: number, to: number, amount: number): number => from + (to - from) * amount;
const mixRgb = (from: Rgb, to: Rgb, amount: number): Rgb => [mixNumber(from[0], to[0], amount), mixNumber(from[1], to[1], amount), mixNumber(from[2], to[2], amount)];

export function lightingAt(phase: number): Lighting {
  checkPhase(phase);
  const upper = keyframes.findIndex((keyframe) => keyframe.phase > phase);
  const from = keyframes[upper - 1];
  const to = keyframes[upper];
  const linear = (phase - from.phase) / (to.phase - from.phase);
  const amount = linear * linear * (3 - 2 * linear);
  const color = (key: "multiply" | "shadowInk" | "highlightInk" | "skyTop" | "skyHorizon"): Rgb => mixRgb(hexColor(from[key]), hexColor(to[key]), amount);
  const value = (key: "inkAmount" | "lightsOn" | "stars" | "shadowStrength"): number => mixNumber(from[key], to[key], amount);
  return {
    phase,
    multiply: color("multiply"), shadowInk: color("shadowInk"), highlightInk: color("highlightInk"), inkAmount: value("inkAmount"),
    skyTop: color("skyTop"), skyHorizon: color("skyHorizon"), lightsOn: value("lightsOn"), stars: value("stars"), shadowStrength: value("shadowStrength"),
  };
}

/**
 * The colour grade, as the world shader applies it. `light` is how much a surface keeps its own colour once the lights
 * are on: 1 for bulbs, partly for lamp-lit paths and guests, 0 for everything else.
 */
export function gradeColor(color: Rgb, lighting: Lighting, light = 0): Rgb {
  const luminance = 0.299 * color[0] + 0.587 * color[1] + 0.114 * color[2];
  const tone = Math.min(1, Math.max(0, (luminance - 0.05) / 0.9));
  const duotone = mixRgb(lighting.shadowInk, lighting.highlightInk, tone * tone * (3 - 2 * tone));
  const lit: Rgb = [color[0] * lighting.multiply[0], color[1] * lighting.multiply[1], color[2] * lighting.multiply[2]];
  return mixRgb(mixRgb(lit, duotone, lighting.inkAmount), color, light * lighting.lightsOn);
}

/** The same grade in GLSL, for the world shader. Uniform names match the `Lighting` fields. */
export const GRADE_GLSL = `
  uniform vec3 gradeMultiply; uniform vec3 gradeShadowInk; uniform vec3 gradeHighlightInk; uniform float gradeInkAmount; uniform float gradeLightsOn;
  vec3 grade(vec3 color, float light) {
    float tone = smoothstep(0.0, 1.0, clamp((dot(color, vec3(0.299, 0.587, 0.114)) - 0.05) / 0.9, 0.0, 1.0));
    vec3 graded = mix(color * gradeMultiply, mix(gradeShadowInk, gradeHighlightInk, tone), gradeInkAmount);
    return mix(graded, color, light * gradeLightsOn);
  }`;
