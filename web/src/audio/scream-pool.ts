export function screamSlot(groups: readonly ({ start: number; end: number } | null)[], time: number): number {
  const free = groups.findIndex((group) => group === null || group.end <= time);
  if (free >= 0) return free;
  return groups.reduce<number>((oldest, group, index) => group!.start < groups[oldest]!.start ? index : oldest, 0);
}
