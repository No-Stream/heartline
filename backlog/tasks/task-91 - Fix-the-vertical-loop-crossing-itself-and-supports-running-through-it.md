---
id: TASK-91
title: Fix the vertical loop crossing itself and supports running through it
status: Done
assignee: []
created_date: '2026-11-04 05:08'
updated_date: '2026-11-04 08:16'
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

2026-11-04: merged 80a14d0. The loop's exit shifts 4 m sideways (left and right loops), supports avoid loops (none under inverted sections; routed columns on wooden loops). The two-train block-section failure was a search weakness: a valid layout existed but the search ran out of budget; it now drops brake runs that fail force checks alone and tries brake pairs after powered sections first. Budget and assertions unchanged. Rust workspace and Vitest pass on master; full gate pending until the journey speed work lands.
<!-- SECTION:NOTES:END -->
