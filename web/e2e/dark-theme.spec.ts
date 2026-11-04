import { expect, test, type Page } from "@playwright/test";
import { openTool } from "./game-page";

test.use({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });

type Rgb = [number, number, number];
const luminance = ([red, green, blue]: Rgb): number => (0.299 * red + 0.587 * green + 0.114 * blue) / 255;

async function startGame(page: Page): Promise<void> {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
}

function themeToggle(page: Page) {
  return page.getByRole("button", { name: "Dark interface", exact: true });
}

async function cssColor(page: Page, selector: string, property: "color" | "backgroundColor"): Promise<Rgb> {
  const value = await page.locator(selector).first().evaluate((element, name) => getComputedStyle(element)[name], property);
  const channels = value.match(/\d+(\.\d+)?/g);
  if (!channels || channels.length < 3) throw new Error(`Unexpected colour ${value}`);
  return channels.slice(0, 3).map(Number) as Rgb;
}

/** Shows a corner of the 512-tile demo park at noon: the north corner has sky above it, the south one empty page around it. */
async function showParkCornerAtNoon(page: Page, corner: "north" | "south"): Promise<void> {
  await page.evaluate((tile) => {
    window.gameTest.setCamera({ centerX: tile, centerY: tile, zoom: 0.5, rotation: 0 });
    window.gameTest.setTimeOfDay(0.5);
  }, corner === "north" ? 4 : 508);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
}

/** Canvas pixels at fractions of its size, read from the drawing buffer the test mode keeps. */
async function canvasPixels(page: Page, points: readonly { x: number; y: number }[]): Promise<Rgb[]> {
  return page.evaluate(async (fractions) => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    const canvas = document.querySelector("canvas")!;
    const gl = canvas.getContext("webgl2")!;
    return fractions.map(({ x, y }) => {
      const color = new Uint8Array(4);
      gl.readPixels(Math.floor(x * canvas.width), Math.floor((1 - y) * canvas.height), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, color);
      return [color[0], color[1], color[2]] as [number, number, number];
    });
  }, points);
}

const skyPoints = [{ x: 0.1, y: 0.05 }, { x: 0.5, y: 0.05 }, { x: 0.9, y: 0.05 }, { x: 0.1, y: 0.5 }];
const pagePoints = [{ x: 0.05, y: 0.95 }, { x: 0.95, y: 0.95 }, { x: 0.5, y: 0.95 }, { x: 0.1, y: 0.5 }];
const parkPoints = [{ x: 0.5, y: 0.7 }, { x: 0.5, y: 0.2 }];

test.describe("dark interface", () => {
  test("the theme toggle works while the atlas is still loading", async ({ page }) => {
    let releaseAtlas: () => void = () => {};
    const atlasReleased = new Promise<void>((resolve) => { releaseAtlas = resolve; });
    await page.route("**/atlas.json", async (route) => {
      await atlasReleased;
      await route.continue();
    });
    await page.goto("/?test=1", { waitUntil: "domcontentloaded" });
    await themeToggle(page).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    releaseAtlas();
    await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  });
  test("stays light without a system preference", async ({ page }) => {
    await startGame(page);
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await expect(themeToggle(page)).toHaveAttribute("aria-pressed", "false");
    expect(luminance(await cssColor(page, ".game-bar", "backgroundColor"))).toBeGreaterThan(0.8);
  });

  test("follows a dark system preference", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await startGame(page);
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    await expect(themeToggle(page)).toHaveAttribute("aria-pressed", "true");
    for (const selector of ["body", ".game-bar"]) expect(luminance(await cssColor(page, selector, "backgroundColor"))).toBeLessThan(0.06);
    expect(luminance(await cssColor(page, ".game-bar h1", "color"))).toBeGreaterThan(0.8);
  });

  test("the toggle switches the theme and the choice outlasts a reload and the system preference", async ({ page }) => {
    await startGame(page);
    await themeToggle(page).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    await expect(themeToggle(page)).toHaveAttribute("aria-pressed", "true");
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    await page.emulateMedia({ colorScheme: "dark" });
    await themeToggle(page).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await expect(themeToggle(page)).toHaveAttribute("aria-pressed", "false");
  });

  test("darkens the sky and the page around the park but leaves the park itself alone", async ({ page }) => {
    await startGame(page);
    const pixelsAround = async (): Promise<{ sky: Rgb[]; page: Rgb[]; park: Rgb[] }> => {
      await showParkCornerAtNoon(page, "north");
      const sky = await canvasPixels(page, skyPoints);
      const northPark = await canvasPixels(page, parkPoints.slice(0, 1));
      await showParkCornerAtNoon(page, "south");
      return { sky, page: await canvasPixels(page, pagePoints), park: [...northPark, ...await canvasPixels(page, parkPoints.slice(1))] };
    };
    const light = await pixelsAround();
    for (const pixel of [...light.sky, ...light.page]) expect(luminance(pixel)).toBeGreaterThan(0.6);

    await themeToggle(page).click();
    const dark = await pixelsAround();
    for (const pixel of dark.sky) expect(luminance(pixel)).toBeLessThan(0.2);
    for (const pixel of dark.page) expect(luminance(pixel)).toBeLessThan(0.06);
    expect(dark.park).toEqual(light.park);
  });

  test.describe("screenshots", () => {
    test.use({ colorScheme: "dark" });

    test("the coaster workshop beside the park", async ({ page }) => {
      await startGame(page);
      await openTool(page, "Coaster");
      await page.addStyleTag({ content: "[data-testid='tick-count'] { visibility: hidden; }" });
      await page.mouse.move(0, 0);
      await expect(page.locator("main")).toHaveScreenshot("dark-coaster-workshop.png", { animations: "disabled" });
    });

    test("the sky and page around the park at noon", async ({ page }) => {
      await startGame(page);
      await showParkCornerAtNoon(page, "north");
      await expect(page.locator("canvas")).toHaveScreenshot("dark-park-edge-noon.png", { animations: "disabled", threshold: 0.1, maxDiffPixelRatio: 0.001 });
    });
  });
});
