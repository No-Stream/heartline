---
id: TASK-19
title: Add scenery and theme sets that affect guests
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 17:34'
labels:
  - art
  - world
  - guests
milestone: m-2
dependencies:
  - TASK-4
  - TASK-14
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: medium
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Theming is a first-class building activity. Start with a few charming theme sets. Guest theme preferences connect scenery to enjoyment. Design sections 6 and 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 At least two theme sets of scenery and building pieces can be placed
- [x] #2 Tests show guests' theme preferences and theming care change their enjoyment
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Garden and Woodland sets, Scenery tool, guest theming responses and demo lamps committed in d65b724. Closing waits on a green full gate (four coaster browser tests time out under parallel load; fix in progress).

Closed 2026-11-03 after two consecutive green make ci runs.
<!-- SECTION:NOTES:END -->
