import { expect, type Page } from "@playwright/test";
import type { TestingReport } from "../../src/generated/TestingReport";

function hasOnlyFiniteNumbers(value: unknown): boolean {
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(hasOnlyFiniteNumbers);
  if (value !== null && typeof value === "object") return Object.values(value).every(hasOnlyFiniteNumbers);
  return true;
}

export async function expectRideReportExplained(page: Page): Promise<TestingReport> {
  await expect(page.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  const report = await page.evaluate(() => window.gameTest.testingReport());
  expect(report).not.toBeNull();
  if (!report) throw new Error("Missing coaster test report");

  expect(hasOnlyFiniteNumbers(report)).toBe(true);
  const contributionCount = Object.values(report.breakdown).reduce((count, contributions) => count + contributions.length, 0);
  expect(contributionCount).toBeGreaterThan(0);
  for (const [category, contributions] of Object.entries(report.breakdown)) {
    await expect(page.locator(`[data-score-breakdown="${category}"] li`)).toHaveCount(contributions.length);
  }

  const astmContributions = Object.values(report.breakdown).flat().filter(({ term }) => term === "envelope");
  if (astmContributions.length > 0) {
    await expect(page.locator("[data-score-breakdown]").filter({ hasText: /ASTM/i })).not.toHaveCount(0);
  }
  for (const note of report.notes.filter((entry) => /ASTM/i.test(entry))) {
    await expect(page.locator("[data-notes]")).toContainText(note);
  }

  return report;
}
