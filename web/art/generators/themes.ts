// Every theme by name. Safe to import in the browser, so the demo can read the scene settings.
import type { Theme, ThemeName } from "./theme.ts";
import { CANDY } from "./themes/candy.ts";
import { MID_CENTURY } from "./themes/mid-century.ts";
import { PEN_AND_WASH } from "./themes/pen-and-wash.ts";
import { RISO } from "./themes/riso.ts";

export const THEMES: Record<ThemeName, Theme> = {
  candy: CANDY,
  riso: RISO,
  "pen-and-wash": PEN_AND_WASH,
  "mid-century": MID_CENTURY,
};

export function themeNamed(name: string): Theme {
  if (!(name in THEMES)) throw new Error(`Unknown theme "${name}"; expected one of ${Object.keys(THEMES).join(", ")}`);
  return THEMES[name as ThemeName];
}
