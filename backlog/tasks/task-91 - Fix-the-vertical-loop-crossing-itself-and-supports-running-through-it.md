---
id: TASK-91
title: Fix the vertical loop crossing itself and supports running through it
status: In Progress
assignee: []
created_date: '2026-11-04 05:08'
updated_date: '2026-11-04 05:24'
labels:
  - coaster
  - art
dependencies: []
priority: high
ordinal: 91000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner screenshot 2026-11-03 (night view of a loop): the loop's exit runs through its entry, and support columns rise through the inside of the loop. Cause found: crates/sim/src/track/curves.rs draws the loop as a circle in the plane of travel with no sideways shift (Shape::Loop uses forward and vertical axes only), so entry and exit coincide; real loops shift sideways by about a track width. Supports are dropped straight down from every track point, including the top of the loop, through the track beneath. Fixing the loop moves its exit, so circuits containing loops need rechecking.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Codex job task-mw1o97te-m8d6r7, worktree theme-park-sim-worktrees/loop-geometry.
<!-- SECTION:NOTES:END -->
