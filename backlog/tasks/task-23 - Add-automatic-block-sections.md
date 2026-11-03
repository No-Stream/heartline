---
id: TASK-23
title: Add automatic block sections
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 14:12'
labels:
  - coaster
  - interface
milestone: m-2
dependencies:
  - TASK-11
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The owner skipped block zones in RCT1 because of the complexity. The game finds positions and places blocks on request, showing how many trains can run. Experts can adjust. Trains wait rather than collide. Design section 5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One action places sensible block sections on test layouts and reports train capacity
- [ ] #2 Experts can move blocks by hand
- [x] #3 Tests confirm trains never collide at any block layout; poor layouts only cost capacity
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Simulation and bridge committed. Remaining: the builder UI button, block markers and capacity report display, and manual block moves in the panel. Placement searches generated brake positions within a budget; throughput excludes loading time.
<!-- SECTION:NOTES:END -->
