// Builds the alternative art themes and writes screenshots for comparing them.
//
// Usage: themes.ts [theme ...]
// Rebuilds the drawings and atlas of each named theme (all alternative themes when none are named)
// under web/art/themes/<theme>/, then screenshots every theme, candy included, into
// web/art/screenshots/themes/, with comparison.png showing COMPARISON_THEMES side by side.
// Candy comes from the game's own atlas, so run `make art` first.
import { execFile } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import type { ThemeName } from "../generators/theme.ts";
import { THEMES } from "../generators/themes.ts";

const ART_DIRECTORY = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUTPUT_DIRECTORY = join(ART_DIRECTORY, "screenshots", "themes");
const PORT = 4182;
const NODE_ARGUMENTS = ["--experimental-strip-types", "--disable-warning=ExperimentalWarning"];
const ALL_THEMES = Object.keys(THEMES);
/** The themes shown side by side in comparison.png, in reading order. */
const COMPARISON_THEMES: readonly ThemeName[] = ["candy", "riso", "mid-century", "print-modern"];
/** The part of the 2x scene shown side by side, in screenshot pixels: buildings, trees, the fountain and a crowd. */
const COMPARISON_CROP = { x: 330, y: 110, width: 760, height: 500 };

const runNode = promisify(execFile);

async function buildTheme(themeName: string): Promise<void> {
  const themeDirectory = join(ART_DIRECTORY, "themes", themeName);
  rmSync(themeDirectory, { recursive: true, force: true });
  const drawings = join(themeDirectory, "drawings");
  await runNode(process.execPath, [...NODE_ARGUMENTS, join(ART_DIRECTORY, "generators", "generate.ts"), themeName, drawings]);
  const { stdout } = await runNode(process.execPath, [...NODE_ARGUMENTS, join(ART_DIRECTORY, "pipeline", "build-atlas.ts"), drawings, join(themeDirectory, "atlas")]);
  console.info(`${themeName}: ${stdout.trim()}`);
}

const requested = process.argv.slice(2);
for (const name of requested) {
  if (!(name in THEMES) || name === "candy") throw new Error(`Cannot build theme "${name}"; candy is built by make art`);
}
await Promise.all((requested.length > 0 ? requested : ALL_THEMES.filter((name) => name !== "candy")).map(buildTheme));

const server = await createServer({ root: ART_DIRECTORY, configFile: false, server: { port: PORT, strictPort: true, host: "127.0.0.1" }, logLevel: "error" });
await server.listen();
const browser = await chromium.launch({ channel: "chrome", args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1480, height: 1000 } });
const problems: string[] = [];
page.on("pageerror", (error) => problems.push(error.message));
page.on("console", (message) => { if (message.type() === "error") problems.push(message.text()); });

for (const themeName of ALL_THEMES) {
  const themeOutput = join(OUTPUT_DIRECTORY, themeName);
  mkdirSync(themeOutput, { recursive: true });
  await page.goto(`http://127.0.0.1:${PORT}/demo.html?theme=${themeName}`);
  await page.waitForSelector("html[data-ready=true]");
  await page.waitForTimeout(2000);
  const scene = page.locator("#scene");
  await scene.screenshot({ path: join(themeOutput, "scene_1x.png") });
  await page.click('[data-zoom="2"]');
  await page.waitForTimeout(600);
  await scene.screenshot({ path: join(themeOutput, "scene_2x.png") });
  await page.click('[data-category="people"]');
  await page.waitForTimeout(400);
  await page.locator("#gallery").screenshot({ path: join(themeOutput, "people.png") });
  for (const category of ["buildings", "scenery"]) {
    await page.click(`[data-category="${category}"]`);
    await page.waitForTimeout(400);
    await page.locator("#gallery").screenshot({ path: join(themeOutput, `${category}.png`) });
  }
}

// The same crop of the compared themes' 2x scenes in a grid, with the theme name above each.
const sources = Object.fromEntries(
  COMPARISON_THEMES.map((name) => [name, `data:image/png;base64,${readFileSync(join(OUTPUT_DIRECTORY, name, "scene_2x.png")).toString("base64")}`]),
);
await page.setContent("<html><body style='margin:0'></body></html>");
const comparison = await page.evaluate(
  async ({ sources, crop }) => {
    const names = Object.keys(sources);
    const columns = 2;
    const [labelHeight, gap] = [34, 12];
    const canvas = document.createElement("canvas");
    canvas.width = columns * crop.width + (columns + 1) * gap;
    canvas.height = Math.ceil(names.length / columns) * (crop.height + labelHeight + gap) + gap;
    const context = canvas.getContext("2d")!;
    context.fillStyle = "#f4efe6";
    context.fillRect(0, 0, canvas.width, canvas.height);
    for (const [index, name] of names.entries()) {
      const image = new Image();
      image.src = sources[name];
      await image.decode();
      const left = gap + (index % columns) * (crop.width + gap);
      const top = gap + Math.floor(index / columns) * (crop.height + labelHeight + gap);
      context.fillStyle = "#2b2730";
      context.font = "600 20px sans-serif";
      context.fillText(name, left, top + 23);
      context.drawImage(image, crop.x, crop.y, crop.width, crop.height, left, top + labelHeight, crop.width, crop.height);
    }
    return canvas.toDataURL("image/png");
  },
  { sources, crop: COMPARISON_CROP },
);
writeFileSync(join(OUTPUT_DIRECTORY, "comparison.png"), Buffer.from(comparison.split(",")[1], "base64"));

await browser.close();
await server.close();
if (problems.length > 0) throw new Error(`The demo reported errors:\n${problems.join("\n")}`);
console.info(`Theme screenshots written to ${OUTPUT_DIRECTORY}`);
