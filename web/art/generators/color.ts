// Colour arithmetic shared by the themes. Colours are always "#rrggbb" strings.

type Rgb = readonly [number, number, number];

function parse(hexColor: string): Rgb {
  if (!/^#[0-9a-f]{6}$/i.test(hexColor)) throw new Error(`Expected a #rrggbb colour, got "${hexColor}"`);
  const value = parseInt(hexColor.slice(1), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function format(channels: Rgb): string {
  return `#${channels.map((channel) => Math.max(0, Math.min(255, Math.round(channel))).toString(16).padStart(2, "0")).join("")}`;
}

/** Linear blend: 0 gives `first`, 1 gives `second`. */
export function mix(first: string, second: string, amount: number): string {
  const [a, b] = [parse(first), parse(second)];
  return format([0, 1, 2].map((index) => a[index] * (1 - amount) + b[index] * amount) as unknown as Rgb);
}

/** Two transparent inks printed on top of each other. */
export function multiply(first: string, second: string): string {
  const [a, b] = [parse(first), parse(second)];
  return format([0, 1, 2].map((index) => (a[index] * b[index]) / 255) as unknown as Rgb);
}

function toLinear(channel: number): number {
  const scaled = channel / 255;
  return scaled <= 0.04045 ? scaled / 12.92 : ((scaled + 0.055) / 1.055) ** 2.4;
}

function fromLinear(channel: number): number {
  const clamped = Math.max(0, Math.min(1, channel));
  return 255 * (clamped <= 0.0031308 ? clamped * 12.92 : 1.055 * clamped ** (1 / 2.4) - 0.055);
}

/** OKLab lightness, a and b, which makes perceptual distance and chroma changes straightforward. */
export function toOklab(hexColor: string): [number, number, number] {
  const [red, green, blue] = parse(hexColor).map(toLinear);
  const long = Math.cbrt(0.4122214708 * red + 0.5363325363 * green + 0.0514459929 * blue);
  const medium = Math.cbrt(0.2119034982 * red + 0.6806995451 * green + 0.1073969566 * blue);
  const short = Math.cbrt(0.0883024619 * red + 0.2817188376 * green + 0.6299787005 * blue);
  return [
    0.2104542553 * long + 0.793617785 * medium - 0.0040720468 * short,
    1.9779984951 * long - 2.428592205 * medium + 0.4505937099 * short,
    0.0259040371 * long + 0.7827717662 * medium - 0.808675766 * short,
  ];
}

export function fromOklab([lightness, greenRed, blueYellow]: readonly [number, number, number]): string {
  const long = (lightness + 0.3963377774 * greenRed + 0.2158037573 * blueYellow) ** 3;
  const medium = (lightness - 0.1055613458 * greenRed - 0.0638541728 * blueYellow) ** 3;
  const short = (lightness - 0.0894841775 * greenRed - 1.291485548 * blueYellow) ** 3;
  return format([
    fromLinear(4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short),
    fromLinear(-1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short),
    fromLinear(-0.0041960863 * long - 0.7034186147 * medium + 1.707614701 * short),
  ]);
}

export function perceptualDistance(first: string, second: string): number {
  const [a, b] = [toOklab(first), toOklab(second)];
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

/** Scales chroma and moves lightness towards a target, keeping hue. */
export function regrade(hexColor: string, { chroma = 1, lightnessTarget = 0, lightnessPull = 0, hueShift = 0 }: { chroma?: number; lightnessTarget?: number; lightnessPull?: number; hueShift?: number }): string {
  const [lightness, greenRed, blueYellow] = toOklab(hexColor);
  const hue = Math.atan2(blueYellow, greenRed) + hueShift;
  const radius = Math.hypot(greenRed, blueYellow) * chroma;
  return fromOklab([lightness + (lightnessTarget - lightness) * lightnessPull, Math.cos(hue) * radius, Math.sin(hue) * radius]);
}

/**
 * A theme's design-to-theme colour function: explicit choices for the colours that matter most,
 * the theme's own colours unchanged, and `fallback` for everything else.
 */
export function paletteMap(overrides: Readonly<Record<string, string>>, themeColors: readonly string[], fallback: (designColor: string) => string): (designColor: string) => string {
  const own = new Set(themeColors.map((color) => color.toLowerCase()));
  const cache = new Map<string, string>();
  const normalizedOverrides = new Map(Object.entries(overrides).map(([design, themed]) => [design.toLowerCase(), themed]));
  return (designColor) => {
    const key = designColor.toLowerCase();
    let themed = cache.get(key);
    if (themed === undefined) {
      themed = normalizedOverrides.get(key) ?? (own.has(key) ? key : fallback(key));
      cache.set(key, themed);
    }
    return themed;
  };
}

/** The colour in `gamut` nearest to `color`, with hue and chroma weighted against lightness. */
export function nearestIn(gamut: readonly string[], color: string, chromaWeight = 1): string {
  const [lightness, greenRed, blueYellow] = toOklab(color);
  let best = gamut[0];
  let bestDistance = Infinity;
  for (const candidate of gamut) {
    const [candidateLightness, candidateGreenRed, candidateBlueYellow] = toOklab(candidate);
    const distance = (lightness - candidateLightness) ** 2 + chromaWeight * ((greenRed - candidateGreenRed) ** 2 + (blueYellow - candidateBlueYellow) ** 2);
    if (distance < bestDistance) [best, bestDistance] = [candidate, distance];
  }
  return best;
}
