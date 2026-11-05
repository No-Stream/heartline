import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { canvasToPage, flattenTerrainTiles, layPath, layQueue, openRide, openTool, setCoasterTicketPrice, setFacilityPrice, setGameSpeed } from "../player/driver";
import { queuedGuests } from "../player/state";
import { parseReplaySession } from "../../src/session-replay";

interface CanvasPoint {
  readonly x: number;
  readonly y: number;
}

interface QueueGuestVisibility {
  readonly guestIds: readonly string[];
  readonly panelInterceptedGuestIds: readonly string[];
}

async function findGuestCanvasPoint(page: Page, guestId: string): Promise<CanvasPoint | null> {
  return page.evaluate((id): CanvasPoint | null => {
    const guest = window.gameTest.guests().find((entry) => entry.id === id);
    const canvas = document.querySelector("canvas");
    if (!guest || !canvas) return null;

    const projected = window.gameTest.project({ x: guest.x, y: guest.y, z: guest.height / 4 });
    const regionX = Math.floor(projected.x) - 32;
    const regionY = Math.floor(projected.y) - 64;
    const regionWidth = 65;
    const regionHeight = 73;
    const picks = window.gameTest.pickRegion(regionX, regionY, regionWidth, regionHeight);
    for (let row = 0; row < regionHeight; row++) {
      for (let column = 0; column < regionWidth; column++) {
        const pick = picks[row * regionWidth + column];
        if (pick && "id" in pick && pick.id === id) {
          return { x: regionX + column + 0.5, y: regionY + row + 0.5 };
        }
      }
    }
    return null;
  }, guestId);
}

async function pointerTarget(page: Page, point: CanvasPoint): Promise<string> {
  const pagePoint = await canvasToPage(page, point);
  return page.evaluate(({ x, y }) => {
    const target = document.elementFromPoint(x, y);
    if (target instanceof HTMLCanvasElement) return "canvas";
    if (target?.closest("[data-testid='guest-panel']")) return "guest-panel";
    return target?.tagName.toLowerCase() ?? "no-element";
  }, pagePoint);
}

async function visibleQueueGuests(page: Page, guestIds: readonly string[]): Promise<QueueGuestVisibility> {
  const visibleGuestIds: string[] = [];
  const panelInterceptedGuestIds: string[] = [];
  for (const guestId of guestIds) {
    const point = await findGuestCanvasPoint(page, guestId);
    if (!point) continue;
    const target = await pointerTarget(page, point);
    if (target === "canvas") visibleGuestIds.push(guestId);
    else if (target === "guest-panel") panelInterceptedGuestIds.push(guestId);
  }
  return { guestIds: visibleGuestIds, panelInterceptedGuestIds };
}

async function clickGuestOnCanvas(page: Page, guestId: string): Promise<void> {
  const point = await findGuestCanvasPoint(page, guestId);
  if (!point) throw new Error(`Guest ${guestId} has no visible renderer pick in the paused queue`);
  const target = await pointerTarget(page, point);
  if (target !== "canvas") {
    throw new Error(`The click point for guest ${guestId} is intercepted by ${target}; no canvas selection was sent`);
  }
  const pagePoint = await canvasToPage(page, point);
  await page.mouse.click(pagePoint.x, pagePoint.y);
}

test("selects each visible queued guest with the guest panel open and closed @journey", async ({ page }) => {
  test.setTimeout(300_000);
  const session = parseReplaySession(JSON.parse(readFileSync(new URL("../sessions/timber-tumbler-built.json", import.meta.url), "utf8")) as unknown);
  await page.setViewportSize({ width: 1280, height: 720 });
  const params = new URLSearchParams({ test: "1", manualBlockAnalysis: "1", park: session.park, seed: session.seed, agents: String(session.guestCount) });
  await page.goto(`/?${params}`);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate((recordedSession) => window.gameTest.replaySession(recordedSession), session);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready), { timeout: 30_000 }).toBe(true);
  await setGameSpeed(page, 1);

  await openTool(page, "Facilities");
  const entrance = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "entrance"));
  if (!entrance) throw new Error("The park entrance is missing");
  await setFacilityPrice(page, { id: entrance.id, name: "entrance" }, 7);
  await page.getByRole("button", { name: `Raise the price of entrance ${entrance.id}` }).click();
  await expect(page.getByRole("spinbutton", { name: `Price of entrance ${entrance.id}` })).toHaveValue("8");

  await flattenTerrainTiles(page, [
    ...Array.from({ length: 6 }, (_, index) => 255 - index).flatMap((y, index) => [238, 244].map((x) => ({ x, y, height: Math.min(12, 8 + index) }))),
    { x: 239, y: 250, height: 12 }, { x: 243, y: 250, height: 12 },
  ]);
  await setGameSpeed(page, 16);
  await layPath(page, { x: 218, y: 256, z: 2 }, { x: 244, y: 256, z: 2 });
  await layQueue(page, { x: 243, y: 250, z: 3 }, { x: 244, y: 256, z: 2 });
  await layPath(page, { x: 239, y: 250, z: 3 }, { x: 238, y: 256, z: 2 });
  await setCoasterTicketPrice(page, 8);
  await page.getByRole("button", { name: "Raise the coaster ticket price" }).click();
  await expect(page.getByRole("spinbutton", { name: "Coaster ticket price" })).toHaveValue("9");
  await openRide(page);
  await expect(page.getByRole("button", { name: "Close coaster" })).toBeVisible();

  await openTool(page, "Watch");
  await setGameSpeed(page, 16);
  await expect.poll(async () => (await queuedGuests(page)).length, { timeout: 45_000 }).toBeGreaterThanOrEqual(3);
  await page.getByRole("button", { name: "Pause" }).click();
  await expect(page.locator("[data-day-clock]")).toHaveAttribute("data-paused", "true");
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
  await page.evaluate(() => window.gameTest.setCamera({ centerX: 241.5, centerY: 253.5, zoom: 1.5, rotation: 0 }));
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  const queueGuestIds = await queuedGuests(page);
  expect(queueGuestIds.length, "the paused coaster queue should still be crowded").toBeGreaterThanOrEqual(2);
  const firstVisible = await visibleQueueGuests(page, queueGuestIds);
  expect(firstVisible.guestIds.length, `visible canvas guests; guest panel blocked ${firstVisible.panelInterceptedGuestIds.join(", ")}`).toBeGreaterThan(0);
  const panel = page.getByTestId("guest-panel");
  await clickGuestOnCanvas(page, firstVisible.guestIds[0]);
  await expect(panel).toBeVisible();
  await expect(panel).toHaveAttribute("data-guest-id", firstVisible.guestIds[0]);

  const initiallyUnobscured = await visibleQueueGuests(page, queueGuestIds);
  expect(
    initiallyUnobscured.guestIds.length,
    `pickable queue guests with the panel open; the panel physically intercepted ${initiallyUnobscured.panelInterceptedGuestIds.join(", ")}`,
  ).toBeGreaterThanOrEqual(2);
  const scheduledGuestIds = new Set(initiallyUnobscured.guestIds);
  const pendingGuestIds = [...initiallyUnobscured.guestIds];
  const checkedWithPanelOpen = new Set<string>();
  while (pendingGuestIds.length > 0) {
    const guestId = pendingGuestIds.shift()!;
    if (checkedWithPanelOpen.has(guestId)) continue;

    // Re-pick against the current frame because selecting a guest moves the panel.
    // clickGuestOnCanvas also checks the DOM hit target before sending pointer input.
    await clickGuestOnCanvas(page, guestId);
    await expect(panel).toBeVisible();
    await expect(panel).toHaveAttribute("data-guest-id", guestId);
    checkedWithPanelOpen.add(guestId);

    await page.getByTestId("guest-panel-close").click();
    await expect(panel).toBeHidden();
    await clickGuestOnCanvas(page, guestId);
    await expect(panel).toBeVisible();
    await expect(panel).toHaveAttribute("data-guest-id", guestId);

    const currentVisibility = await visibleQueueGuests(page, queueGuestIds);
    for (const visibleGuestId of currentVisibility.guestIds) {
      if (scheduledGuestIds.has(visibleGuestId)) continue;
      scheduledGuestIds.add(visibleGuestId);
      pendingGuestIds.push(visibleGuestId);
    }
  }
  expect(checkedWithPanelOpen.size, "each pickable guest exposed with the panel open should be selectable").toBeGreaterThanOrEqual(2);
});
