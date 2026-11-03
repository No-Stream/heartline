---
id: TASK-10
title: Build the piece-by-piece coaster building tool
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 04:08'
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
- [ ] #1 A browser test builds a small coaster piece by piece, and a train completes a circuit
- [ ] #2 Only pieces valid for the model and current track end are offered
- [ ] #3 Swept-shape clearance allows tight near-misses and refuses real collisions; a support conflict reroutes, and a piece is refused only when no route exists
- [ ] #4 Deleting track with a train on it recovers the train and riders cleanly
- [ ] #5 Owner has tried it and judged that building feels good
<!-- AC:END -->
