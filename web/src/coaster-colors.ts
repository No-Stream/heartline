/**
 * The palette coasters are painted from. The simulation stores only the names (crates/sim/src/track/colors.rs keeps the
 * same list in the same order); the inks live here. They are the print-modern inks plus a few mid tones, all light
 * enough to keep their hue under the night grade.
 */
export const COASTER_COLORS = {
  coral: { label: "Coral", hex: "#dc5c3e" },
  mustard: { label: "Mustard", hex: "#e9ab38" },
  timber: { label: "Timber", hex: "#d88440" },
  walnut: { label: "Walnut", hex: "#9a6440" },
  rose: { label: "Rose", hex: "#e88a9c" },
  plum: { label: "Plum", hex: "#8a5c9e" },
  slate: { label: "Slate", hex: "#4f72b8" },
  sky: { label: "Sky", hex: "#7fb4d8" },
  teal: { label: "Teal", hex: "#2f8f88" },
  sage: { label: "Sage", hex: "#9fbbb1" },
  cream: { label: "Cream", hex: "#f2e6cc" },
  charcoal: { label: "Charcoal", hex: "#4a4552" },
} as const;

export type CoasterColorName = keyof typeof COASTER_COLORS;
export const COASTER_COLOR_NAMES = Object.keys(COASTER_COLORS) as CoasterColorName[];
export const COASTER_PARTS = ["track", "supports", "trains"] as const;
export type CoasterPart = (typeof COASTER_PARTS)[number];
export type CoasterColorScheme = Readonly<Record<CoasterPart, CoasterColorName>>;

/** Steel running rails on wooden track are always this dark steel, whatever the track is painted. */
export const RUNNING_RAIL_HEX = COASTER_COLORS.charcoal.hex;

export function isCoasterColorName(value: string): value is CoasterColorName {
  return Object.hasOwn(COASTER_COLORS, value);
}

/** Validates palette names received from the worker. */
export function parseCoasterColors(value: { track: string; supports: string; trains: string }): CoasterColorScheme {
  for (const part of COASTER_PARTS) if (!isCoasterColorName(value[part])) throw new Error(`Unknown coaster colour "${value[part]}"`);
  return value as CoasterColorScheme;
}

/** The same scheme with one part repainted. */
export function repaint(scheme: CoasterColorScheme, part: CoasterPart, color: string): CoasterColorScheme {
  if (!isCoasterColorName(color)) throw new Error(`Unknown coaster colour "${color}"`);
  return { ...scheme, [part]: color };
}
