import { expect, type Page } from "@playwright/test";

export type ToolName = "Watch" | "Coaster" | "Paths" | "Landscape" | "Facilities" | "Scenery";

/** Open a tool from the toolbar, which shows its panel and switches the input mode. */
export async function openTool(page: Page, tool: ToolName): Promise<void> {
  const button = page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: tool, exact: true });
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
}

/** Convert a point in the world canvas's backing-store pixels, as `gameTest.project` returns it, to page coordinates. */
export async function canvasToPage(page: Page, point: { x: number; y: number }): Promise<{ x: number; y: number }> {
  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  if (!bounds) throw new Error("Canvas is not visible");
  const size = await canvas.evaluate((element: HTMLCanvasElement) => ({ width: element.width, height: element.height }));
  return { x: bounds.x + point.x * bounds.width / size.width, y: bounds.y + point.y * bounds.height / size.height };
}

export async function clickCanvas(page: Page, point: { x: number; y: number }): Promise<void> {
  const target = await canvasToPage(page, point);
  await page.mouse.click(target.x, target.y);
}

export async function moveOverCanvas(page: Page, point: { x: number; y: number }, options?: { steps?: number }): Promise<void> {
  const target = await canvasToPage(page, point);
  await page.mouse.move(target.x, target.y, options);
}

/** The world canvas's backing-store size, which follows the window and the open tool. */
export async function canvasSize(page: Page): Promise<{ width: number; height: number }> {
  return page.locator("canvas").evaluate((element: HTMLCanvasElement) => ({ width: element.width, height: element.height }));
}
