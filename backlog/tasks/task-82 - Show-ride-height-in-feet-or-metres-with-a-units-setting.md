---
id: TASK-82
title: Show ride height in feet or metres with a units setting
status: Done
assignee: []
created_date: '2026-11-04 00:28'
updated_date: '2026-11-04 06:16'
labels:
  - interface
  - coaster
dependencies: []
priority: high
ordinal: 82000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-03: show the coaster's height while building and in the testing report (track end height above ground, maximum height, drop height), in feet by default for this owner, with a metric/imperial setting that also applies to other displayed lengths and speeds (station height stepper, speeds in the testing panel). Store the choice per viewer.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03 night: owner added height markers on the track while building, as important for intuitive building; feet by default, metric supported.

2026-11-03 night: Codex job task-mw1nps3h-h35h3s in worktree theme-park-sim-worktrees/height-markers (branch height-markers).

2026-11-03 night: merged 950e10d. The lead replaced the units dropdown in the game bar with a small ft/m toggle (the dropdown overflowed the bar at 800 px) and dropped unit tests that checked a hand-built fake of the page in favour of the browser spec.
<!-- SECTION:NOTES:END -->
