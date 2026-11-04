---
id: TASK-80
title: Add direct level-to-60-degree lift and drop transitions
status: Done
assignee: []
created_date: '2026-11-04 00:28'
updated_date: '2026-11-04 10:57'
labels:
  - coaster
dependencies: []
priority: high
ordinal: 80000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-03: on Timber Tumbler a 60-degree lift takes two steps (gentle then steep then 60). Steps are fine, but also offer one transition piece from level straight to 60 degrees (and 60 back to level, and the descending mirrors), within the existing vertical-g limits (about 4 g at 25 m/s; a longer piece if needed). For models whose LiftRules allow 60 degrees (Timber Tumbler, Velocity Viper). Not aiming for maximum realism; steeper RMC-style lifts are welcome.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Codex job task-mw1z6m4f-34wppt in worktree theme-park-sim-worktrees/track-batch (with the other two of TASK-80, TASK-81, TASK-83).

2026-11-04: merged 5fad6b4. Lap tests use frictionless physics; forces sampled at 25 m/s (60-degree transitions) and 20 m/s (banked drops).
<!-- SECTION:NOTES:END -->
