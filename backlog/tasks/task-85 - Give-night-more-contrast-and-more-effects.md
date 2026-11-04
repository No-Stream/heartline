---
id: TASK-85
title: Give night more contrast and more effects
status: Done
assignee: []
created_date: '2026-11-04 00:28'
updated_date: '2026-11-04 09:44'
labels:
  - rendering
  - art
dependencies: []
priority: medium
ordinal: 85000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-03: night feels quite grey. Increase contrast in the night grade (deeper shadows, brighter lit areas), and add more night effects: ride lights along track and on flat rides, lit stall signs, path lamp pools, perhaps searchlights or more fireworks. Keep the print-modern look; check both light and dark interface themes and the daylight screenshot baselines.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03 night: owner repeated the request for more night-time contrast.

2026-11-04: Claude Opus subagent in worktree theme-park-sim-worktrees/night-and-colours (night contrast and effects; per-coaster colours stored in the coaster state).

2026-11-04: merged ac9cfc5. Night lamp pools are drawn as two hard-edged flat bands; softer edges are an easy follow-up if the owner prefers. Ties now take the track colour.
<!-- SECTION:NOTES:END -->
