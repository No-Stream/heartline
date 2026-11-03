export type TimingSummary = { meanMs: number; p95Ms: number };

export function summarizeTimings(values: number[]): TimingSummary {
  if (values.length === 0) throw new RangeError("Cannot summarize an empty timing sample");

  const sortedValues = [...values].sort((first, second) => first - second);
  const meanMs = values.reduce((sum, value) => sum + value, 0) / values.length;
  const p95Ms = sortedValues[Math.ceil(sortedValues.length * 0.95) - 1]!;
  return { meanMs, p95Ms };
}
