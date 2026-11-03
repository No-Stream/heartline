import { expect, test, type Locator, type Page } from "@playwright/test";

interface GuestPick {
  readonly id: string;
  readonly x: number;
  readonly y: number;
}

async function openGuestPanel(page: Page): Promise<{ guestId: string; panel: Locator }> {
  await page.goto("/?test=1");
  await expect.poll(() => page.evaluate(() => window.gameTest?.ready), { timeout: 30_000 }).toBe(true);
  await page.evaluate(() => {
    const guest = window.gameTest.guests()[0];
    window.gameTest.setCamera({ centerX: guest.x, centerY: guest.y, zoom: 2, rotation: 0 });
  });
  await expect.poll(() => page.evaluate(() => window.gameTest.ready)).toBe(true);

  const target = await page.evaluate((): GuestPick | null => {
    const canvas = document.querySelector("canvas");
    if (!canvas) return null;
    for (let y = 0; y < canvas.height; y += 4) {
      for (let x = 0; x < canvas.width; x += 4) {
        const picked = window.gameTest.pick(x, y);
        if (picked?.kind === "sprite" && picked.id === "guest-9") {
          return { id: picked.id, x, y };
        }
      }
    }
    return null;
  });
  if (!target) throw new Error("No guest could be picked from the rendered world");

  const canvasBounds = await page.locator("canvas").boundingBox();
  if (!canvasBounds) throw new Error("Game canvas is not visible");
  await page.mouse.click(
    canvasBounds.x + target.x * canvasBounds.width / 800,
    canvasBounds.y + target.y * canvasBounds.height / 500,
  );

  const panel = page.getByTestId("guest-panel");
  await expect(panel).toBeVisible();
  await expect(panel).toHaveAttribute("data-guest-id", target.id);
  return { guestId: target.id, panel };
}

test("opens a guest story, switches to a group member, and closes with Escape", async ({ page }) => {
  const { guestId, panel } = await openGuestPanel(page);
  const guestName = panel.getByTestId("guest-name");
  await expect(guestName).toBeVisible();
  await expect(guestName).not.toBeEmpty();
  await expect(panel.getByTestId("guest-needs")).toBeVisible();
  await expect(panel.getByTestId("guest-thoughts")).toBeVisible();
  await expect(panel.getByTestId("guest-thoughts")).not.toBeEmpty();

  const otherGroupMember = panel.locator(`button[data-guest-id]:not([data-guest-id="${guestId}"])`).first();
  await expect(otherGroupMember).toBeVisible();
  const nextGuestId = await otherGroupMember.getAttribute("data-guest-id");
  if (!nextGuestId) throw new Error("Group member button has no guest id");
  await otherGroupMember.click();
  await expect(panel).toHaveAttribute("data-guest-id", nextGuestId);
  await expect(panel.getByTestId("guest-name")).not.toBeEmpty();

  await page.keyboard.press("Escape");
  await expect(panel).toBeHidden();
});

test("captures the open guest details panel", async ({ page }) => {
  const { panel } = await openGuestPanel(page);
  await expect(panel.getByTestId("guest-name")).not.toBeEmpty();
  await expect(panel.getByTestId("guest-needs")).toBeVisible();
  await expect(panel.getByTestId("guest-thoughts")).toBeVisible();
  await page.locator("canvas").evaluate((canvas) => { canvas.style.visibility = "hidden"; });
  await panel.evaluate((element) => {
    element.style.transition = "none";
    element.style.left = `${Math.round(element.getBoundingClientRect().left)}px`;
    element.style.top = `${Math.round(element.getBoundingClientRect().top)}px`;
  });
  await expect(panel).toHaveScreenshot("guest-details-panel.png", { animations: "disabled" });
});
