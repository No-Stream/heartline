// Drives the demo in headless Chrome and writes review screenshots to web/art/screenshots/.
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { createServer } from "vite";

const ART_DIRECTORY = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUTPUT_DIRECTORY = join(ART_DIRECTORY, "screenshots");
const PORT = 4181;

mkdirSync(OUTPUT_DIRECTORY, { recursive: true });
const server = await createServer({ root: ART_DIRECTORY, configFile: false, server: { port: PORT, strictPort: true, host: "127.0.0.1" }, logLevel: "error" });
await server.listen();
const browser = await chromium.launch({ channel: "chrome", args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1480, height: 1000 } });
const problems: string[] = [];
page.on("pageerror", (error) => problems.push(error.message));
page.on("console", (message) => { if (message.type() === "error") problems.push(message.text()); });
await page.goto(`http://127.0.0.1:${PORT}/demo.html`);
await page.waitForSelector("html[data-ready=true]");
await page.waitForTimeout(1500);

const scene = page.locator("#scene");
const shoot = async (name: string): Promise<void> => {
  await page.waitForTimeout(500);
  await scene.screenshot({ path: join(OUTPUT_DIRECTORY, `${name}.png`) });
};
for (const rotation of [0, 90, 180, 270]) {
  await shoot(`scene_rotation_${rotation}`);
  await page.click("#rotate-right");
}
for (const zoom of [0.5, 2]) {
  await page.click(`[data-zoom="${zoom}"]`);
  await shoot(`scene_zoom_${zoom}x`);
}
await page.click('[data-zoom="1"]');
await page.locator("#stats").waitFor();
for (const category of ["people", "buildings", "scenery", "ground", "sky"]) {
  await page.click(`[data-category="${category}"]`);
  await page.waitForTimeout(300);
  await page.locator("#gallery").screenshot({ path: join(OUTPUT_DIRECTORY, `gallery_${category}.png`) });
}
const stats = await page.locator("#stats").textContent();
console.info(`Demo stats: ${stats}`);
await browser.close();
await server.close();
if (problems.length > 0) throw new Error(`The demo reported errors:\n${problems.join("\n")}`);
console.info(`Screenshots written to ${OUTPUT_DIRECTORY}`);
