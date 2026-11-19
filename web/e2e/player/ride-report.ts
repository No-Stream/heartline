import { expect, type Page } from "@playwright/test";
import type { TestingReport } from "../../src/generated/TestingReport";
import { formatTestingNote } from "../../src/coaster-testing";
import { groupScoreFindings, splitScoreFindings } from "../../src/score-findings";

function hasOnlyFiniteNumbers(value: unknown): boolean {
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(hasOnlyFiniteNumbers);
  if (value !== null && typeof value === "object") return Object.values(value).every(hasOnlyFiniteNumbers);
  return true;
}

export async function expectRideReportExplained(page: Page): Promise<TestingReport> {
  // The ride info window repeats the same report markup, so read the builder's testing panel only.
  const testingPanel = page.locator("[data-construction] [data-testing]");
  await expect(testingPanel.locator("[data-rating]")).toContainText(/Excitement: .*Intensity: .*Nausea:/);
  const report = await page.evaluate(() => window.gameTest.testingReport());
  expect(report).not.toBeNull();
  if (!report) throw new Error("Missing coaster test report");

  expect(hasOnlyFiniteNumbers(report)).toBe(true);
  const contributionCount = Object.values(report.breakdown).reduce((count, contributions) => count + contributions.length, 0);
  expect(contributionCount).toBeGreaterThan(0);
  const { main, more } = splitScoreFindings(groupScoreFindings(report.breakdown));
  await expect(testingPanel.locator("[data-score-findings] [data-score-finding]")).toHaveCount(main.length);
  await expect(testingPanel.locator("[data-score-findings-more] [data-score-finding]")).toHaveCount(more.length);
  await expect(testingPanel.locator("[data-score-findings] [data-score-finding]").first().locator(".testing-effect")).not.toHaveCount(0);

  const envelope = [...main, ...more].find(({ key }) => key === "envelope");
  if (envelope) {
    await expect(testingPanel.locator('[data-score-finding="envelope"]')).toHaveCount(1);
    await expect(testingPanel.locator('[data-score-finding="envelope"] .testing-finding__text')).toContainText("ASTM");
  }
  const selectedUnits = await page.locator("[data-units]").getAttribute("data-units");
  if (selectedUnits !== "imperial" && selectedUnits !== "metric") {
    throw new Error(`Unknown selected units in ride report: ${selectedUnits}`);
  }
  for (const note of report.notes.filter((entry) => /ASTM/i.test(entry))) {
    await expect(testingPanel.locator("[data-notes]")).toContainText(formatTestingNote(note, selectedUnits));
  }

  return report;
}
