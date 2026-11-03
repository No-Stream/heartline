---
id: TASK-24
title: Add coaster proximity events
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 13:06'
labels:
  - coaster
  - guests
milestone: m-2
dependencies:
  - TASK-16
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Near-misses, headchoppers, dueling sections, tunnels and terrain hugging feed ratings and rider memories, using the swept-shape data. This rewards the stacking the owner loves. Design section 4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each event type is detected in test layouts
- [x] #2 Events raise ride rating and appear in rider memories
- [x] #3 Detection cost is included in the benchmark
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Simulation side committed. Remaining: show events in the testing panel and phrase the new thoughts in the guest panel (web wiring, after the day-night work frees web/src).
<!-- SECTION:NOTES:END -->
