// Classifies a cast member into the appearance keys used by the renderer.
import type { PersonAppearance, PersonDesign, HairStyle } from "./person-design.ts";

const HAIR_GROUPS: Record<HairStyle, PersonAppearance["hairGroup"]> = {
  afro: "curly", shortCrop: "short", bun: "tied", bald: "bald", braid: "tied", bowl: "short",
  pigtails: "tied", puffs: "curly", hijab: "covered", wavyBob: "bob", swoop: "short", balding: "bald", highBun: "tied",
};

function rgb(hex: string): [number, number, number] {
  return [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16)) as [number, number, number];
}

function hueGroup(color: string): PersonAppearance["clothingPaletteGroup"] {
  const [red, green, blue] = rgb(color);
  const maximum = Math.max(red, green, blue);
  const minimum = Math.min(red, green, blue);
  if (maximum - minimum < 28) return "neutral";
  const delta = maximum - minimum;
  const hue = maximum === red ? ((green - blue) / delta + (green < blue ? 6 : 0)) * 60
    : maximum === green ? ((blue - red) / delta + 2) * 60
      : ((red - green) / delta + 4) * 60;
  if (hue < 15 || hue >= 345) return "red";
  if (hue < 45) return "orange";
  if (hue < 75) return "yellow";
  if (hue < 165) return "green";
  if (hue < 255) return "blue";
  return "purple";
}

export function personAppearance(person: PersonDesign): PersonAppearance {
  const [red, green, blue] = rgb(person.skin);
  const brightness = (red * 299 + green * 587 + blue * 114) / 1000;
  const widthRatio = (person.shoulderHalfWidth + person.hipHalfWidth + person.belly * 0.3) / person.height;
  return {
    ageGroup: person.ageGroup,
    skinToneGroup: brightness < 100 ? "deep" : brightness < 180 ? "medium" : "light",
    hairGroup: HAIR_GROUPS[person.hairStyle],
    clothingPaletteGroup: hueGroup(person.top),
    bodyShape: widthRatio < 0.17 ? "slender" : widthRatio < 0.24 ? "average" : "broad",
  };
}
