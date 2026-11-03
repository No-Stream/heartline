---
id: TASK-10
title: Build the piece-by-piece coaster building tool
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 05:15'
labels:
  - coaster
  - interface
milestone: m-1
dependencies:
  - TASK-8
  - TASK-9
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The heart of the game: placing pieces one at a time should feel delightful. Clearance uses the swept shape of the train with generous tolerance, so tight near-misses work. Design section 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A browser test builds a small coaster piece by piece, and a train completes a circuit
- [x] #2 Only pieces valid for the model and current track end are offered
- [x] #3 Swept-shape clearance allows tight near-misses and refuses real collisions; a support conflict reroutes, and a piece is refused only when no route exists
- [x] #4 Deleting track with a train on it recovers the train and riders cleanly
- [ ] #5 Owner has tried it and judged that building feels good
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Builder fixes committed in 14b38a9; choices refresh in 37 ms on a 150-piece circuit. Waiting for the owner to try it (last criterion).
<!-- SECTION:NOTES:END -->
