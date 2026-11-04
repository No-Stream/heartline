---
id: TASK-87
title: Let players choose coaster colours
status: Done
assignee: []
created_date: '2026-11-04 05:08'
updated_date: '2026-11-04 09:44'
labels:
  - coaster
  - art
dependencies: []
priority: medium
ordinal: 87000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-03: per-coaster colours for track, supports and trains, chosen in the construction panel; today colours come only from the interface theme, so every coaster looks the same.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-04: Claude Opus subagent in worktree theme-park-sim-worktrees/night-and-colours (night contrast and effects; per-coaster colours stored in the coaster state).

2026-11-04: merged ac9cfc5. Night lamp pools are drawn as two hard-edged flat bands; softer edges are an easy follow-up if the owner prefers. Ties now take the track colour.
<!-- SECTION:NOTES:END -->
