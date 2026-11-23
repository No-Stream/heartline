import { expect, test, type Page } from "@playwright/test";
import { COASTER_MODELS } from "../../src/generated/coaster-models";
import {
  clickVisibleButton,
  closeRideWindow,
  flattenTerrainTiles,
  finishCoaster,
  layPath,
  layQueue,
  openRide,
  placeStation,
  renameSelectedCoaster,
  selectCoasterByName,
  selectCoasterFromWorld,
  showBuilderTab,
  startPark,
  stopCoasterEditing,
  testRide,
} from "../player/driver";
import { placeCircuit } from "../player/place-circuit";
import { readRide } from "../player/rides";

test("players can start, select, edit and demolish coasters independently @journey", async ({ page }) => {
  test.setTimeout(360_000);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.setViewportSize({ width: 1280, height: 720 });
  await startPark(page, { park: "new", seed: 2027, speed: 1, testMode: true, agents: 48 });

  const daisyDash = readRide("daisy-dash");
  await placeStation(page, {
    model: "family_coaster",
    height: 4,
    tile: { x: 241, y: 249, z: 2 },
    direction: { x: 241, y: 251, z: 2 },
  });
  await placeCircuit(page, daisyDash.steps);
  await finishCoaster(page);
  await expect(page.locator("[data-station-access]")).toContainText("queue at (243, 250)");
  await expect(page.locator("[data-station-access]")).toContainText("normal exit path at (239, 250)");
  await connectDaisyDashToPark(page);

  await stopCoasterEditing(page);
  await selectCoasterByName(page, "Daisy Dash");
  await testAndOpenSelectedRide(page);
  await closeRideWindow(page);

  const smallCoaster = readRide("browser-agreement-fast");
  const smallCoasterModel = COASTER_MODELS.find(({ key }) => key === smallCoaster.model);
  if (!smallCoasterModel) throw new Error(`Ride model ${smallCoaster.model} is missing from the coaster catalog`);
  await placeStation(page, {
    model: smallCoasterModel.key,
    height: 4,
    tile: { x: 280, y: 300, z: 2 },
    direction: { x: 280, y: 302, z: 2 },
  });
  await placeCircuit(page, smallCoaster.steps);
  await finishCoaster(page);
  await expect(page.locator("[data-station-access]")).toContainText("queue at (282, 301)");
  await expect(page.locator("[data-station-access]")).toContainText("normal exit path at (278, 301)");
  await connectSmallCoasterToPark(page);
  await stopCoasterEditing(page);
  await selectCoasterByName(page, smallCoasterModel.name);
  await renameSelectedCoaster(page, smallCoaster.name);
  await testAndOpenSelectedRide(page);

  const coasterList = page.locator(".coaster-list");
  await closeRideWindow(page);
  await expect(coasterList).toBeVisible();
  const listedCoasters = coasterList.locator("[data-coaster-id]");
  await expect(listedCoasters).toHaveCount(2);
  await expect(listedCoasters.nth(0)).toHaveAttribute("aria-label", /, Open$/);
  await expect(listedCoasters.nth(1)).toHaveAttribute("aria-label", /, Open$/);
  await expect(coasterList).toHaveScreenshot("multi-coaster-list.png", { animations: "disabled" });
  await selectCoasterByName(page, "Daisy Dash");
  await expect(page.getByTestId("ride-window")).toHaveScreenshot("multi-coaster-ride-window.png", { animations: "disabled" });
  await closeRideWindow(page);

  const rideEvidence = await collectRideEvidence(page, 60_000);
  const rideEvidenceSummary = {
    simulationTicks: rideEvidence.simulationTicks,
    wallTimeMs: Number(rideEvidence.wallTimeMs.toFixed(1)),
    millisecondsPerTick: Number((rideEvidence.wallTimeMs / rideEvidence.simulationTicks).toFixed(4)),
    queuedCoasters: [...rideEvidence.queuedCoasters],
    peakRiderCounts: rideEvidence.peakRiderCounts,
    finalRiderCounts: rideEvidence.finalRiderCounts,
    boardedCoasters: [...rideEvidence.boardedCoasters],
    completedCoasters: [...rideEvidence.completedCoasters],
    completedGuestIds: rideEvidence.completedGuestIds,
  };
  await test.info().attach("two-coaster-simulation-timing", {
    body: JSON.stringify(rideEvidenceSummary, null, 2),
    contentType: "application/json",
  });
  const rideEvidenceDiagnostic = JSON.stringify(rideEvidenceSummary);
  expect([...rideEvidence.queuedCoasters].sort(), `guests queued for both rides; evidence=${rideEvidenceDiagnostic}`).toEqual([0, 1]);
  expect([...rideEvidence.boardedCoasters].sort(), `guests rode both coasters; evidence=${rideEvidenceDiagnostic}`).toEqual([0, 1]);
  expect([...rideEvidence.completedCoasters].sort(), `guest ride memories recorded both completed rides; evidence=${rideEvidenceDiagnostic}`).toEqual([0, 1]);

  for (const name of ["Daisy Dash", smallCoaster.name]) {
    await selectCoasterByName(page, name);
    await expect.poll(async () => Number(await page.getByTestId("ride-income").textContent()), {
      message: `${name} earned ticket income independently`,
    }).toBeGreaterThan(0);
    await closeRideWindow(page);
  }

  await closeRideWindow(page);
  await selectCoasterFromWorld(page, 0, { centerX: 241, centerY: 249 });
  await expect(page.getByTestId("ride-window")).toHaveAttribute("data-ride", "0");
  await closeRideWindow(page);
  await selectCoasterFromWorld(page, 1, { centerX: 280, centerY: 300 });
  await expect(page.getByTestId("ride-window")).toHaveAttribute("data-ride", "1");
  await closeRideWindow(page);
  await selectCoasterByName(page, "Daisy Dash");

  await clickVisibleButton(page.getByTestId("ride-edit"));
  await expect.poll(() => page.evaluate(() => window.gameTest.editingCoasterId())).toBe(0);
  const movingTicks = await waitForTrainMovement(page, 1, 15_000);
  await test.info().attach("second-coaster-while-editing", {
    body: JSON.stringify({ coasterId: 1, simulationTicks: movingTicks }),
    contentType: "application/json",
  });
  await showBuilderTab(page, "Colours");
  const trainColors = page.getByRole("radiogroup", { name: "Trains colour" });
  const selectedTrainColor = trainColors.locator('[role="radio"][aria-checked="true"]');
  await expect(selectedTrainColor).toHaveCount(1);
  const previousTrainColor = await selectedTrainColor.getAttribute("aria-label");
  if (previousTrainColor === null) throw new Error("Selected coaster train colour has no name");
  const nextTrainColor = previousTrainColor === "Coral" ? "Teal" : "Coral";
  const changedTrainColor = trainColors.getByRole("radio", { name: nextTrainColor, exact: true });
  await clickVisibleButton(changedTrainColor);
  await expect(changedTrainColor).toHaveAttribute("aria-checked", "true");
  await showBuilderTab(page, "Build");
  await testRide(page);
  await stopCoasterEditing(page);
  await selectCoasterByName(page, "Daisy Dash");
  await openRide(page);
  await expect(page.getByTestId("ride-open-toggle")).toHaveAccessibleName("Close coaster");

  await closeRideWindow(page);
  await selectCoasterByName(page, smallCoaster.name);
  await clickVisibleButton(page.getByTestId("ride-demolish"));
  await expect(page.getByTestId("ride-demolish-confirm")).toBeVisible();
  await clickVisibleButton(page.getByTestId("ride-demolish-cancel"));
  await expect.poll(() => page.evaluate(() => window.gameTest.coasters().map(({ id }) => id))).toEqual([0, 1]);
  await clickVisibleButton(page.getByTestId("ride-demolish"));
  await clickVisibleButton(page.getByTestId("ride-demolish-confirm"));
  await advanceTicks(page, 0);
  await expect.poll(() => page.evaluate(() => window.gameTest.coasters().map(({ id }) => id))).toEqual([0]);

  await selectCoasterByName(page, "Daisy Dash");
  await expect(page.getByTestId("ride-open-toggle")).toHaveAccessibleName("Close coaster");
  const incomeBeforeDemolition = Number(await page.getByTestId("ride-income").textContent());
  const postDemolitionTicks = await waitForIncomeIncrease(page, incomeBeforeDemolition, 30_000);
  await test.info().attach("remaining-coaster-after-demolition", {
    body: JSON.stringify({ coasterId: 0, simulationTicks: postDemolitionTicks }),
    contentType: "application/json",
  });
  expect(pageErrors).toEqual([]);
});

async function testAndOpenSelectedRide(page: Page): Promise<void> {
  const rideWindow = page.getByTestId("ride-window");
  await expect(rideWindow).toBeVisible();
  await clickVisibleButton(rideWindow.getByTestId("ride-test"));
  await expect(rideWindow.getByTestId("ride-report")).toBeVisible({ timeout: 60_000 });
  await clickVisibleButton(rideWindow.getByTestId("ride-open-toggle"));
  await expect(rideWindow.getByTestId("ride-open-toggle")).toHaveAccessibleName("Close coaster");
}

interface TickAdvanceEvidence {
  readonly queueEntryPlaceIds: readonly number[];
}

async function advanceTicks(page: Page, ticks: number): Promise<TickAdvanceEvidence> {
  return page.evaluate(async (count) => {
    const evidence = await window.gameTest.advanceTicks(count);
    await new Promise<void>((resolve) => {
      const waitForFrame = (): void => {
        if (window.gameTest.ready) resolve();
        else requestAnimationFrame(waitForFrame);
      };
      waitForFrame();
    });
    return evidence;
  }, ticks);
}

interface RideEvidence {
  readonly queuedCoasters: ReadonlySet<number>;
  readonly boardedCoasters: ReadonlySet<number>;
  readonly completedCoasters: ReadonlySet<number>;
  readonly completedGuestIds: readonly (readonly string[])[];
  readonly peakRiderCounts: readonly [number, number];
  readonly finalRiderCounts: readonly [number, number];
  readonly simulationTicks: number;
  readonly wallTimeMs: number;
}

async function collectRideEvidence(page: Page, maximumTicks: number): Promise<RideEvidence> {
  const queuedCoasters = new Set<number>();
  const boardedCoasters = new Set<number>();
  const completedCoasters = new Set<number>();
  const passengerIdsByCoaster = new Map<number, Set<string>>([[0, new Set()], [1, new Set()]]);
  const completedGuestIds: [string[], string[]] = [[], []];
  const peakRiderCounts: [number, number] = [0, 0];
  const finalRiderCounts: [number, number] = [0, 0];
  const initialAdvanceStartedAt = performance.now();
  const initialQueuePeaks = await advanceTicks(page, 1);
  let simulationTicks = 1;
  let wallTimeMs = performance.now() - initialAdvanceStartedAt;

  const recordQueueEvidence = (batch: TickAdvanceEvidence): void => {
    for (const placeId of batch.queueEntryPlaceIds) {
      if (placeId === 0xffff_ffff) queuedCoasters.add(0);
      if (placeId === 0xffff_fffe) queuedCoasters.add(1);
    }
  };
  recordQueueEvidence(initialQueuePeaks);

  while (simulationTicks < maximumTicks && (queuedCoasters.size < 2 || completedCoasters.size < 2)) {
    const startedAt = performance.now();
    const ticks = Math.min(100, maximumTicks - simulationTicks);
    const batchQueuePeaks = await advanceTicks(page, ticks);
    wallTimeMs += performance.now() - startedAt;
    simulationTicks += ticks;
    recordQueueEvidence(batchQueuePeaks);

    const observation = await page.evaluate(() => {
      const coasters = window.gameTest.coasters().filter(({ id }) => id <= 1).map(({ id, riders }) => {
        const guestIds: string[] = [];
        for (let offset = 2; offset < riders.length; offset += 3) guestIds.push(`guest-${riders[offset]}`);
        return { id, guestIds };
      });
      const guestIds = [...new Set([
        ...window.gameTest.guests().map(({ id }) => id),
        ...coasters.flatMap(({ guestIds: riders }) => riders),
      ])];
      return { coasters, guestIds };
    });

    for (const coasterId of [0, 1] as const) {
      const current = observation.coasters.find(({ id }) => id === coasterId)?.guestIds ?? [];
      finalRiderCounts[coasterId] = current.length;
      peakRiderCounts[coasterId] = Math.max(peakRiderCounts[coasterId], current.length);
      const observed = passengerIdsByCoaster.get(coasterId)!;
      for (const guestId of current) {
        boardedCoasters.add(coasterId);
        observed.add(guestId);
      }
      if (completedCoasters.has(coasterId)) continue;
      const currentIds = new Set(current);
      const candidate = [...observed].find((guestId) => !currentIds.has(guestId) && observation.guestIds.includes(guestId));
      if (!candidate) continue;
      await page.evaluate((guestId) => window.gameTest.requestGuestDetail(guestId), candidate);
      const placeId = 0xffff_ffff - coasterId;
      await expect.poll(() => page.evaluate(({ guestId, rideId }) => {
        const detail = window.gameTest.guestDetail();
        return detail?.id === guestId && detail.memories.some((memory) => memory.kind === "ride" && memory.rideId === rideId);
      }, { guestId: candidate, rideId: placeId }), {
        message: `guest ${candidate} completed coaster ${coasterId}`,
        timeout: 15_000,
      }).toBe(true);
      completedCoasters.add(coasterId);
      completedGuestIds[coasterId].push(candidate);
    }
  }

  return {
    queuedCoasters,
    boardedCoasters,
    completedCoasters,
    completedGuestIds,
    peakRiderCounts,
    finalRiderCounts,
    simulationTicks,
    wallTimeMs,
  };
}

async function waitForTrainMovement(page: Page, coasterId: number, maximumTicks: number): Promise<number> {
  const readTrainPoses = async (): Promise<number[]> => page.evaluate((id) => {
    const coaster = window.gameTest.coasters().find(({ id: currentId }) => currentId === id);
    if (!coaster) throw new Error(`Coaster ${id} disappeared while it should be running`);
    return Array.from(coaster.trainPoses);
  }, coasterId);
  let previous = await readTrainPoses();
  for (let simulationTicks = 100; simulationTicks <= maximumTicks; simulationTicks += 100) {
    await advanceTicks(page, 100);
    const current = await readTrainPoses();
    if (current.length > 0 && current.some((value, index) => value !== previous[index])) return simulationTicks;
    previous = current;
  }
  throw new Error(`Coaster ${coasterId} did not move within ${maximumTicks} simulation ticks while another coaster was being edited`);
}

async function waitForIncomeIncrease(page: Page, previousIncome: number, maximumTicks: number): Promise<number> {
  for (let simulationTicks = 500; simulationTicks <= maximumTicks; simulationTicks += 500) {
    await advanceTicks(page, 500);
    const currentIncome = Number(await page.getByTestId("ride-income").textContent());
    if (currentIncome > previousIncome) return simulationTicks;
  }
  throw new Error(`The remaining coaster did not earn new income within ${maximumTicks} simulation ticks`);
}

async function connectDaisyDashToPark(page: Page): Promise<void> {
  await flattenTerrainTiles(page, stationAccessTerrain(249, 238, 244, 239, 243));
  await layPath(page, { x: 218, y: 251, z: 2 }, { x: 218, y: 256, z: 2 });
  await layPath(page, { x: 218, y: 256, z: 2 }, { x: 244, y: 256, z: 2 });
  await layQueue(page, { x: 243, y: 250, z: 3 }, { x: 244, y: 256, z: 2 });
  await layPath(page, { x: 239, y: 250, z: 3 }, { x: 238, y: 256, z: 2 });
}

async function connectSmallCoasterToPark(page: Page): Promise<void> {
  await flattenTerrainTiles(page, stationAccessTerrain(300, 277, 283, 278, 282));
  await layPath(page, { x: 240, y: 256, z: 2 }, { x: 240, y: 268, z: 2 });
  await layPath(page, { x: 240, y: 268, z: 2 }, { x: 240, y: 280, z: 2 });
  await layPath(page, { x: 240, y: 280, z: 2 }, { x: 240, y: 292, z: 2 });
  await layPath(page, { x: 240, y: 292, z: 2 }, { x: 240, y: 307, z: 2 });
  await layPath(page, { x: 218, y: 256, z: 2 }, { x: 218, y: 268, z: 2 });
  await layPath(page, { x: 218, y: 268, z: 2 }, { x: 218, y: 280, z: 2 });
  await layPath(page, { x: 218, y: 280, z: 2 }, { x: 218, y: 292, z: 2 });
  await layPath(page, { x: 218, y: 292, z: 2 }, { x: 218, y: 307, z: 2 });
  await layPath(page, { x: 218, y: 307, z: 2 }, { x: 230, y: 307, z: 2 });
  await layPath(page, { x: 230, y: 307, z: 2 }, { x: 240, y: 307, z: 2 });
  await layPath(page, { x: 240, y: 307, z: 2 }, { x: 252, y: 307, z: 2 });
  await layPath(page, { x: 252, y: 307, z: 2 }, { x: 264, y: 307, z: 2 });
  await layPath(page, { x: 264, y: 307, z: 2 }, { x: 276, y: 307, z: 2 });
  await layPath(page, { x: 276, y: 307, z: 2 }, { x: 282, y: 307, z: 2 });
  await layQueue(page, { x: 282, y: 301, z: 3 }, { x: 283, y: 307, z: 2 });
  await layPath(page, { x: 282, y: 307, z: 2 }, { x: 283, y: 307, z: 2 });
  await layPath(page, { x: 278, y: 301, z: 3 }, { x: 277, y: 307, z: 2 });
  await layPath(page, { x: 282, y: 308, z: 2 }, { x: 282, y: 320, z: 2 });
  await layPath(page, { x: 282, y: 320, z: 2 }, { x: 282, y: 330, z: 2 });
}

function stationAccessTerrain(stationY: number, rampExitX: number, rampQueueX: number, exitOpeningX: number, queueOpeningX: number): { x: number; y: number; height: number }[] {
  return [
    ...Array.from({ length: 6 }, (_, index) => [rampExitX, rampQueueX].map((x) => ({
      x,
      y: stationY + 6 - index,
      height: Math.min(12, 8 + index),
    }))).flat(),
    { x: exitOpeningX, y: stationY + 1, height: 12 },
    { x: queueOpeningX, y: stationY + 1, height: 12 },
  ];
}
