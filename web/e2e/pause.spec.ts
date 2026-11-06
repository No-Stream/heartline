import { expect, test } from "@playwright/test";
import { canvasToPage, moveOverCanvas, openTool } from "./game-page";

test("pausing freezes guests and ticks while tools remain usable", async ({ page }) => {
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  const pause = page.getByRole("button", { name: "Pause" });
  await pause.click();
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
  await expect(page.locator("[data-day-clock]")).toContainText("Paused");
  const before = await page.evaluate(() => ({ tick: window.gameTest.tickCount(), guest: window.gameTest.guests()[0] }));
  await page.waitForTimeout(1_000);
  const after = await page.evaluate(() => ({ tick: window.gameTest.tickCount(), guest: window.gameTest.guests()[0] }));
  expect(after.tick).toBe(before.tick);
  expect({ x: after.guest.x, y: after.guest.y, height: after.guest.height }).toEqual({ x: before.guest.x, y: before.guest.y, height: before.guest.height });

  await page.evaluate(() => window.gameTest.setCamera({ centerX: 50, centerY: 50, zoom: 1, rotation: 0 }));
  await openTool(page, "Paths");
  await page.getByRole("button", { name: "Build paths" }).click();
  const pathPoints = await page.evaluate(() => [
    window.gameTest.project({ x: 48.5, y: 51.5, z: 2 }),
    window.gameTest.project({ x: 48.5, y: 47.5, z: 2 }),
  ]);
  await moveOverCanvas(page, pathPoints[0]);
  const pathStart = await canvasToPage(page, pathPoints[0]);
  await page.mouse.move(pathStart.x, pathStart.y);
  await page.mouse.down();
  const pathEnd = await canvasToPage(page, pathPoints[1]);
  await page.mouse.move(pathEnd.x, pathEnd.y, { steps: 8 });
  await page.mouse.up();
  await expect(page.getByRole("region", { name: "Paths" }).getByRole("status")).toHaveText("Path built.");
  expect(await page.evaluate(() => window.gameTest.paths().length)).toBeGreaterThan(0);

  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Watch", exact: true }).click();
  await page.locator("canvas").focus();
  await page.keyboard.press("Space");
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "false");
  await expect.poll(() => page.evaluate(() => window.gameTest.tickCount()), { timeout: 5_000 }).toBeGreaterThan(before.tick);
});

test("Resume works with a guest panel open at different game bar widths", async ({ page }) => {
  await page.goto("/");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);

  await page.getByRole("button", { name: "Pause" }).click();
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
  const guestId = await page.evaluate(() => {
    const guest = window.gameTest.guests()[0];
    window.gameTest.setCamera({ centerX: guest.x, centerY: guest.y, zoom: 2, rotation: 0 });
    return guest.id;
  });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  const guestPoint = await page.evaluate((id) => {
    const canvas = document.querySelector("canvas")!;
    const guest = window.gameTest.guests().find((entry) => entry.id === id)!;
    const projected = window.gameTest.project({ x: guest.x, y: guest.y, z: guest.height / 4 });
    const left = Math.max(0, Math.floor(projected.x) - 36);
    const top = Math.max(0, Math.floor(projected.y) - 72);
    const right = Math.ceil(Math.min(canvas.width, projected.x + 36));
    const bottom = Math.ceil(Math.min(canvas.height, projected.y + 8));
    const width = right - left;
    const height = bottom - top;
    if (width < 1 || height < 1) return null;

    const picks = window.gameTest.pickRegion(left, top, width, height);
    for (let y = top; y < bottom; y += 2) {
      for (let x = left; x < right; x += 2) {
        const picked = picks[(y - top) * width + x - left];
        if (picked?.kind === "sprite" && picked.id === id) {
          const directPick = window.gameTest.pick(x, y);
          if (JSON.stringify(directPick) !== JSON.stringify(picked)) {
            throw new Error("Batched and point picking disagree for the visible guest");
          }
          return { x, y };
        }
      }
    }
    return null;
  }, guestId);
  if (!guestPoint) throw new Error("No guest could be picked from the rendered world");
  const point = await canvasToPage(page, guestPoint);
  await page.mouse.click(point.x, point.y);
  const panel = page.getByTestId("guest-panel");
  await expect(panel).toHaveAttribute("data-guest-id", guestId);

  let previousFitLevel = await page.locator(".game-bar").getAttribute("data-fit-level");
  for (const scenario of [
    { width: 800, height: 480, focusPanel: false },
    { width: 520, height: 480, focusPanel: true },
    { width: 1280, height: 720, focusPanel: false },
  ]) {
    await page.setViewportSize({ width: scenario.width, height: scenario.height });
    await expect.poll(() => page.locator(".game-bar").getAttribute("data-fit-level")).not.toBe(previousFitLevel);
    previousFitLevel = await page.locator(".game-bar").getAttribute("data-fit-level");
    if (scenario.focusPanel) await panel.getByTestId("guest-panel-close").focus();
    else await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());

    const tickBeforeResume = await page.evaluate(() => window.gameTest.tickCount());
    const guestBeforeResume = await page.evaluate((id) => window.gameTest.guests().find((guest) => guest.id === id)!, guestId);
    const resume = page.getByRole("button", { name: "Resume" });
    const hitDetails = await resume.evaluate((button) => {
      const bounds = button.getBoundingClientRect();
      const topElement = document.elementFromPoint(bounds.left + bounds.width / 2, bounds.top + bounds.height / 2);
      const panelBounds = document.querySelector<HTMLElement>("[data-testid='guest-panel']")!.getBoundingClientRect();
      const canvasBounds = document.querySelector("canvas")!.getBoundingClientRect();
      return {
        target: topElement === button || button.contains(topElement) ? "resume" : topElement?.closest("[data-testid='guest-panel']") ? "guest-panel" : topElement?.tagName ?? "none",
        button: { left: bounds.left, top: bounds.top, right: bounds.right, bottom: bounds.bottom },
        panel: { left: panelBounds.left, top: panelBounds.top, right: panelBounds.right, bottom: panelBounds.bottom },
        canvas: { left: canvasBounds.left, top: canvasBounds.top, right: canvasBounds.right, bottom: canvasBounds.bottom },
      };
    });
    expect(hitDetails.target, `Resume hit target at ${scenario.width}px: ${JSON.stringify(hitDetails)}`).toBe("resume");
    await resume.click();
    await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "false");
    await expect.poll(() => page.evaluate(() => window.gameTest.tickCount()), { timeout: 5_000 }).toBeGreaterThan(tickBeforeResume);
    await expect.poll(async () => {
      const current = await page.evaluate((id) => window.gameTest.guests().find((guest) => guest.id === id)!, guestId);
      return current.x !== guestBeforeResume.x || current.y !== guestBeforeResume.y || current.height !== guestBeforeResume.height;
    }, { timeout: 10_000 }).toBe(true);

    if (scenario.width !== 1280) {
      await page.getByRole("button", { name: "Pause" }).click();
      await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
    }
  }
});
