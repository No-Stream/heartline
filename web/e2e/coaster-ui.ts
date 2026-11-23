import { expect, type Page } from "@playwright/test";
import { openTool } from "./game-page";

/** Open the coaster builder, following the New coaster entry when the tool opens on the park ride list. */
export async function openCoasterBuilder(page: Page): Promise<void> {
  await openTool(page, "Coaster");
  const newCoaster = page.getByRole("button", { name: "New coaster", exact: true });
  if (await newCoaster.isVisible()) await newCoaster.click();
  await expect(page.getByRole("region", { name: "Coaster construction", exact: true })).toBeVisible();
}
