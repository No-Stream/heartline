---
id: TASK-23
title: Add automatic block sections
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 17:34'
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
- [x] #1 One action places sensible block sections on test layouts and reports train capacity
- [x] #2 Experts can move blocks by hand
- [x] #3 Tests confirm trains never collide at any block layout; poor layouts only cost capacity
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Builder UI committed in 4c805b5: automatic placement, train stepper, capacity and throughput, draggable markers. Closing waits on the same green gate.

Closed 2026-11-03 after two consecutive green make ci runs; the four slow coaster browser tests pass under the full parallel suite.
<!-- SECTION:NOTES:END -->
