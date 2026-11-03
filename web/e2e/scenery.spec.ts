import { expect, test } from "@playwright/test";
import { clickCanvas, openTool } from "./game-page";

test.use({ deviceScaleFactor: 1, launchOptions: { channel: "chrome", args: ["--use-gl=angle", "--use-angle=swiftshader"] } });

test("places and removes scenery from both theme sets @slow", async ({ page }) => {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await openTool(page, "Scenery");
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 150.5, centerY: 150.5, zoom: 1, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const panel = page.getByRole("region", { name: "Scenery" });
  const point = await page.evaluate(() => window.gameTest.project({ x: 150.25, y: 150.25, z: 2 }));
  await panel.getByRole("button", { name: "Lamp", exact: true }).click();
  await panel.getByRole("button", { name: "Turn scenery clockwise" }).click();
  await panel.getByRole("button", { name: "Place scenery" }).click();
  await clickCanvas(page, point);
  await expect(panel.getByRole("status")).toHaveText("Scenery placed.");
  await panel.getByRole("button", { name: "Remove scenery" }).click();
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const spritePoint = await page.evaluate((p) => {
    for (let dy = 0; dy >= -50; dy--) {
      const pick = window.gameTest.pick(p.x, p.y + dy);
      if (pick && "id" in pick && pick.id.startsWith("scenery-")) return { x: p.x, y: p.y + dy };
    }
    throw new Error("Placed scenery is not pickable");
  }, point);
  await clickCanvas(page, spritePoint);
  await expect(panel.getByRole("status")).toHaveText("Scenery removed.");
  await panel.getByRole("button", { name: "Tree Cone", exact: true }).click();
  await panel.getByRole("button", { name: "Place scenery" }).click();
  await clickCanvas(page, point);
  await expect(panel.getByRole("status")).toHaveText("Scenery placed.");
});
