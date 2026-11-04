---
id: TASK-94
title: Round station height and other inputs to whole feet in imperial units
status: Done
assignee: []
created_date: '2026-11-04 09:44'
updated_date: '2026-11-04 10:16'
labels:
  - interface
dependencies: []
priority: medium
ordinal: 94000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seen 2026-11-04: with feet selected, the station height field shows 39.37 ft (a straight conversion of 12 m) instead of a whole-foot value with a whole-foot step. Inputs should step and display in sensible imperial increments while the simulation keeps metres.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Codex job task-mw1y7lu5-ur3mxm in worktree theme-park-sim-worktrees/feet-rounding.

2026-11-04: merged 6b45440.
<!-- SECTION:NOTES:END -->
