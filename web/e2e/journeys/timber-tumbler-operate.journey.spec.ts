import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { advanceUntilState, canvasToPage, flattenTerrainTiles, layPath, layQueue, openRide, openTool, setCoasterTicketPrice, setFacilityPrice, setGameSpeed } from "../player/driver";
import { queuedGuests, revenue, riders } from "../player/state";
import { parseReplaySession } from "../../src/session-replay";

async function coasterQueuingGuests(page: Page): Promise<readonly string[]> {
  const queuingIds: string[] = [];
  for (const id of await queuedGuests(page)) {
    await page.evaluate((guestId) => window.gameTest.requestGuestDetail(guestId), id);
    await page.evaluate(() => window.gameTest.advanceTicks(0));
    const detail = await page.evaluate(() => window.gameTest.guestDetail());
    if (detail?.id !== id) throw new Error(`Guest detail for ${id} was not received`);
    if (detail.activity.kind === "queuing" && detail.activity.placeId === 0xffff_ffff) queuingIds.push(id);
  }
  return queuingIds;
}

async function selectQueuedGuest(page: Page): Promise<string> {
  await page.getByRole("toolbar", { name: "Tools" }).getByRole("button", { name: "Watch", exact: true }).click();
  await setGameSpeed(page, 16);
  await setGameSpeed(page, 1);
  await page.getByRole("button", { name: "Pause" }).click();
  await advanceUntilState(page, async () => (await coasterQueuingGuests(page)).length > 0, { ticks: 21_600 });
  const seenIds = new Set<string>();
  for (;;) {
    const queuedIds = await coasterQueuingGuests(page);
    for (const id of queuedIds) seenIds.add(id);
    for (const id of queuedIds) {
      const guest = await page.evaluate((guestId) => window.gameTest.guests().find((entry) => entry.id === guestId), id);
      if (!guest) continue;
      await page.evaluate(({ x, y }) => window.gameTest.setCamera({ centerX: x, centerY: y, zoom: 2, rotation: 0 }), guest);
      await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);
      const point = await page.evaluate((guestId) => {
          const current = window.gameTest.guests().find((entry) => entry.id === guestId);
          if (!current) return null;
          const projected = window.gameTest.project({ x: current.x, y: current.y, z: current.height / 4 });
          const offsets: { dx: number; dy: number }[] = [];
          for (let dy = -64; dy <= 8; dy += 2) for (let dx = -32; dx <= 32; dx += 2) offsets.push({ dx, dy });
          offsets.sort((first, second) => first.dx ** 2 + (first.dy + 12) ** 2 - second.dx ** 2 - (second.dy + 12) ** 2);
          const regionX = Math.floor(projected.x) - 34, regionY = Math.floor(projected.y) - 66;
          const regionWidth = 69, regionHeight = 77;
          const picks = window.gameTest.pickRegion(regionX, regionY, regionWidth, regionHeight);
          for (const { dx, dy } of offsets) {
            const x = projected.x + dx, y = projected.y + dy;
            const matches = [[0, 0], [-2, 0], [2, 0], [0, -2], [0, 2]].every(([offsetX, offsetY]) => {
              const column = Math.floor(x + offsetX) - regionX, row = Math.floor(y + offsetY) - regionY;
              const pick = picks[row * regionWidth + column];
              return pick && "id" in pick && pick.id === guestId;
            });
            if (matches) return { x, y };
          }
          return null;
      }, id);
      if (!point) continue;
      const pickedId = await page.evaluate(({ x, y }) => {
        const pick = window.gameTest.pick(x, y);
        return pick && "id" in pick ? pick.id : null;
      }, point);
      if (pickedId !== id) continue;
      const clickPoint = await canvasToPage(page, point);
      await page.mouse.click(clickPoint.x, clickPoint.y);
      await expect(page.getByTestId("guest-panel")).toHaveAttribute("data-guest-id", id);
      await expect.poll(() => page.evaluate((guestId) => window.gameTest.guestDetail()?.id === guestId, id)).toBe(true);
      const activity = await page.evaluate(() => window.gameTest.guestDetail()!.activity);
      if (activity.kind !== "queuing" || activity.placeId !== 0xffff_ffff) {
        await page.getByTestId("guest-panel-close").click();
        continue;
      }
      expect(activity.kind, "selected guest's coaster activity").toBe("queuing");
      expect(activity.placeId, "selected guest's destination").toBe(0xffff_ffff);
      await page.getByRole("button", { name: "Resume" }).click();
      await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();
      return id;
    }
    await advanceUntilState(page, async () => (await coasterQueuingGuests(page)).some((id) => !seenIds.has(id)), { ticks: 21_600 });
  }
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
  phase("terrain");
  await setGameSpeed(page, 16);
  await page.setViewportSize({ width: 1280, height: 720 });
  await layPath(page, { x: 218, y: 256, z: 2 }, { x: 244, y: 256, z: 2 });
  await layQueue(page, { x: 243, y: 250, z: 3 }, { x: 244, y: 256, z: 2 });
  await layPath(page, { x: 239, y: 250, z: 3 }, { x: 238, y: 256, z: 2 });
  phase("station access");
  await setCoasterTicketPrice(page, 8);
  await page.getByRole("button", { name: "Raise the coaster ticket price" }).click();
  await expect(page.getByRole("spinbutton", { name: "Coaster ticket price" })).toHaveValue("9");
  const revenueBeforeOpening = await revenue(page);
  const crestBurstsBeforeOpening = await page.evaluate(() => window.gameTest.crestBursts().length);
  await openRide(page);
  await expect(page.getByRole("button", { name: "Close coaster" })).toBeVisible();
  phase("prices and opening");
  const guestId = await selectQueuedGuest(page);
  phase("guest selection");
  await expect(page.getByTestId("guest-panel")).toBeVisible();
  await setGameSpeed(page, 16);
  await page.getByRole("button", { name: "Pause" }).click();
  await advanceUntilState(page, async () => (await riders(page)).some(({ id }) => id === guestId), { ticks: 90_000 });
  phase("boarding");
  await advanceUntilState(page, async () => {
    const income = Number(await page.locator("[data-coaster-income]").textContent());
    const detail = await page.evaluate(() => window.gameTest.guestDetail());
    return income > 0 && await revenue(page) > revenueBeforeOpening
      && detail?.id === guestId && detail.memories.some((memory) => memory.kind === "ride" && memory.rideId === 0xffff_ffff)
      && (await page.locator("[data-panel-memories]").textContent())?.includes("Ride ") === true
      && await page.evaluate((count) => window.gameTest.crestBursts().length > count, crestBurstsBeforeOpening);
  }, { ticks: 100_000 });
  phase("ride memory and income");
});
