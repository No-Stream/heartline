import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { canvasToPage, flattenTerrainTiles, layPath, layQueue, openRide, openTool, setCoasterTicketPrice, setFacilityPrice, setGameSpeed, waitForState } from "../player/driver";
import { queuedGuests, revenue, riders } from "../player/state";
import { parseReplaySession } from "../../src/session-replay";

async function selectQueuedGuest(page: Page): Promise<string> {
  const target: { id: string | null } = { id: null };
  const counts = { polls: 0, trainsRunning: 0, candidates: 0, vanishedAfterSlowing: 0, missing: 0, unpickable: 0, movedOrLeftQueue: 0, pickChanged: 0, waiting: 0 };
  const waitingIds = new Set<string>();
  const deadline = Date.now() + 45_000;
  await setGameSpeed(page, 16);
  while (!target.id && Date.now() < deadline) {
    counts.polls++;
    const observedRiders = await riders(page);
    const observedQueue = await queuedGuests(page);
    for (const id of waitingIds) if (!observedQueue.includes(id)) waitingIds.delete(id);
    if (observedRiders.length === 0 || !observedQueue.some((id) => !waitingIds.has(id))) {
      await page.waitForTimeout(100);
      continue;
    }
    counts.trainsRunning++;
    await setGameSpeed(page, 1);
    const queuedIds = await queuedGuests(page);
    if (!queuedIds.some((id) => !waitingIds.has(id))) counts.vanishedAfterSlowing++;
    counts.candidates += queuedIds.length;
    for (const id of queuedIds) {
      if (waitingIds.has(id)) continue;
      const guest = await page.evaluate((guestId) => window.gameTest.guests().find((entry) => entry.id === guestId), id);
      if (!guest) { counts.missing++; continue; }
      await page.evaluate(({ x, y }) => window.gameTest.setCamera({ centerX: x, centerY: y, zoom: 2, rotation: 0 }), guest);
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      const point = await page.evaluate((guestId) => {
        const current = window.gameTest.guests().find((entry) => entry.id === guestId);
        if (!current) return null;
        const projected = window.gameTest.project({ x: current.x, y: current.y, z: current.height / 4 });
        for (let dy = -64; dy <= 8; dy += 2) {
          for (let dx = -32; dx <= 32; dx += 2) {
            const pick = window.gameTest.pick(projected.x + dx, projected.y + dy);
            if (pick && "id" in pick && pick.id === guestId) return { x: projected.x + dx, y: projected.y + dy };
          }
        }
        return null;
      }, id);
      if (!point) { counts.unpickable++; continue; }
      await page.waitForTimeout(100);
      const current = await page.evaluate((guestId) => window.gameTest.guests().find((entry) => entry.id === guestId), id);
      if (!current || !(await queuedGuests(page)).includes(id)
        || current.x !== guest.x || current.y !== guest.y || current.height !== guest.height) {
        counts.movedOrLeftQueue++;
        continue;
      }
      const pickedId = await page.evaluate(({ x, y }) => {
        const pick = window.gameTest.pick(x, y);
        return pick && "id" in pick ? pick.id : null;
      }, point);
      if (pickedId !== id) { counts.pickChanged++; continue; }
      const clickPoint = await canvasToPage(page, point);
      await page.mouse.click(clickPoint.x, clickPoint.y);
      const afterClick = await page.evaluate((guestId) => window.gameTest.guests().find((entry) => entry.id === guestId), id);
      expect(afterClick && { x: afterClick.x, y: afterClick.y, height: afterClick.height }, "queued guest moved during selection").toEqual({ x: current.x, y: current.y, height: current.height });
      await expect(page.getByTestId("guest-panel")).toHaveAttribute("data-guest-id", id);
      await expect.poll(() => page.evaluate((guestId) => window.gameTest.guestDetail()?.id === guestId, id)).toBe(true);
      const activity = await page.evaluate(() => window.gameTest.guestDetail()!.activity);
      expect(activity.placeId, "selected guest's destination").toBe(0xffff_ffff);
      if (activity.kind === "waiting") {
        counts.waiting++;
        waitingIds.add(id);
        await page.getByTestId("guest-panel-close").click();
        continue;
      }
      expect(activity.kind, "selected guest's coaster activity").toBe("queuing");
      target.id = id;
      break;
    }
    if (!target.id) await setGameSpeed(page, 16);
  }
  if (!target.id) throw new Error(`No stationary queued guest was pickable within 45 seconds: ${JSON.stringify(counts)}`);
  return target.id;
}

test("Timber Tumbler operating journey @journey", async ({ page }) => {
  test.setTimeout(180_000);
  let phaseStarted = Date.now();
  const phase = (name: string): void => {
    console.log(`Journey phase ${name}: ${Date.now() - phaseStarted} ms`);
    phaseStarted = Date.now();
  };
  const session = parseReplaySession(JSON.parse(readFileSync(new URL("../sessions/timber-tumbler-built.json", import.meta.url), "utf8")) as unknown);
  await page.setViewportSize({ width: 800, height: 480 });
  const params = new URLSearchParams({ test: "1", manualBlockAnalysis: "1", park: session.park, seed: session.seed, agents: String(session.guestCount) });
  await page.goto(`/?${params}`);
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate((recordedSession) => window.gameTest.replaySession(recordedSession), session);
  await expect.poll(() => page.evaluate(() => window.gameTest.ready), { timeout: 30_000 }).toBe(true);
  await setGameSpeed(page, 1);
  phase("replay setup");
  await flattenTerrainTiles(page, [
    ...Array.from({ length: 6 }, (_, index) => 255 - index).flatMap((y, index) => [238, 244].map((x) => ({ x, y, height: Math.min(12, 8 + index) }))),
    { x: 239, y: 250, height: 12 }, { x: 243, y: 250, height: 12 },
  ]);
  phase("terrain");
  await page.setViewportSize({ width: 1280, height: 720 });
  await layPath(page, { x: 218, y: 256, z: 2 }, { x: 230, y: 256, z: 2 });
  await layPath(page, { x: 230, y: 256, z: 2 }, { x: 238, y: 256, z: 2 });
  await layPath(page, { x: 238, y: 256, z: 2 }, { x: 244, y: 256, z: 2 });
  await layQueue(page, { x: 243, y: 250, z: 3 }, { x: 244, y: 256, z: 2 });
  await layPath(page, { x: 239, y: 250, z: 3 }, { x: 238, y: 256, z: 2 });
  phase("station access");
  await openTool(page, "Facilities");
  const entrance = await page.evaluate(() => window.gameTest.facilities().find(({ kind }) => kind === "entrance"));
  if (!entrance) throw new Error("The park entrance is missing");
  await setFacilityPrice(page, { id: entrance.id, name: "entrance" }, 8);
  await setCoasterTicketPrice(page, 9);
  const revenueBeforeOpening = await revenue(page);
  const crestBurstsBeforeOpening = await page.evaluate(() => window.gameTest.crestBursts().length);
  await openRide(page);
  await expect(page.getByRole("button", { name: "Close coaster" })).toBeVisible();
  phase("prices and opening");
  const guestId = await selectQueuedGuest(page);
  phase("guest selection");
  await expect(page.getByTestId("guest-panel")).toBeVisible();
  await setGameSpeed(page, 16);
  await waitForState(page, async () => (await riders(page)).some(({ id }) => id === guestId), { ticks: 90_000 });
  phase("boarding");
  await waitForState(page, async () => {
    const income = Number(await page.locator("[data-coaster-income]").textContent());
    const detail = await page.evaluate(() => window.gameTest.guestDetail());
    return income > 0 && await revenue(page) > revenueBeforeOpening
      && detail?.id === guestId && detail.memories.some((memory) => memory.kind === "ride" && memory.rideId === 0xffff_ffff)
      && (await page.locator("[data-panel-memories]").textContent())?.includes("Ride ") === true
      && await page.evaluate((count) => window.gameTest.crestBursts().length > count, crestBurstsBeforeOpening);
  }, { ticks: 100_000 });
  phase("ride memory and income");
});
