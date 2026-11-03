---
id: TASK-46
title: Fix path tool placing one tile at a time
status: Done
assignee: []
created_date: '2026-11-03 16:40'
updated_date: '2026-11-03 18:01'
labels:
  - world
  - interface
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: the path tool now seems to place one square at a time instead of dragging a run, likely a regression from the explicit path edge connections or toolbar changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Dragging lays a full path run with a preview again, in every rotation
- [x] #2 A browser test drags a multi-tile run and checks every tile is placed and connected
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The one-tile report was caused by toolbar selection opening the Paths panel without activating path build mode. After fixing toolbar activation, realistic normal-path drags passed at all four rotations and zoom 0.75 and 1.5. The test checks the six-tile preview, placed tiles, and reciprocal explicit edge bits. No separate drag implementation change was needed. Targeted Playwright specs passed.
<!-- SECTION:NOTES:END -->
