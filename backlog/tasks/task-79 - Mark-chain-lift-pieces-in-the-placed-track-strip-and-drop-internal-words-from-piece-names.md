---
id: TASK-79
title: >-
  Mark chain lift pieces in the placed-track strip and drop internal words from
  piece names
status: Done
assignee: []
created_date: '2026-11-03 23:11'
updated_date: '2026-11-04 08:49'
labels:
  - coaster
  - interface
dependencies: []
priority: medium
ordinal: 79000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the TASK-70 palette pass (b4c2e8d): the placed-track strip in the builder does not show which pieces carry a chain, because the construction state from sim-wasm and worker.ts has no per-piece kind. Add the kind to the construction state and mark lift pieces in the strip. Also piece names still show internal words, e.g. 'Climbing standalone small left turn'; give players plain names (e.g. 'Climbing small left turn') without breaking the journeys' locators.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner playtest 2026-11-03: 'Begin gentle descent' and 'Begin downward curve' read alike. The downward curve (VerticalDownEntry/Continue/Exit) is a three-piece smooth pitch from level into a 45-degree drop; the gentle descent tips only to 27 degrees in one piece. Rename the curve family so the steepness and the three-step nature are clear (same for the upward curve).

2026-11-03 night: Codex job task-mw1sbatq-yb7ls7 in worktree theme-park-sim-worktrees/palette-elements (element choices, plain names, chain markers).

2026-11-04: merged 1fbfe0b. Known limit: a climbing or descending turn that needs a slope transition first only tries the shortest transition, so it can hide a turn another transition would allow.
<!-- SECTION:NOTES:END -->
