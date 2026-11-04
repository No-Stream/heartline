---
id: TASK-90
title: Make the construction palette's flow and names intuitive
status: Done
assignee: []
created_date: '2026-11-04 05:08'
updated_date: '2026-11-04 08:49'
labels:
  - coaster
  - construction
  - interface
dependencies: []
priority: high
ordinal: 90000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-03: climbing is confusing (start the curve, place another, then 'begin 60 degree climb'), and 'Standalone small left banked turn' versus 'Begin small left banked turn' is unclear. Needs a design pass on how multi-part elements are offered; overlaps TASK-79 (names) and TASK-80 (direct level-to-60 transitions).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03 night, owner approved: each multi-part element is one palette choice that places its transitions automatically; separate pieces remain for fine control.

2026-11-03 night: Codex job task-mw1sbatq-yb7ls7 in worktree theme-park-sim-worktrees/palette-elements (element choices, plain names, chain markers).

2026-11-04: merged 1fbfe0b. Known limit: a climbing or descending turn that needs a slope transition first only tries the shortest transition, so it can hide a turn another transition would allow.
<!-- SECTION:NOTES:END -->
