---
id: TASK-86
title: Make circuits easy to close with very short pieces and S-bends
status: Done
assignee: []
created_date: '2026-11-04 05:08'
updated_date: '2026-11-04 05:24'
labels:
  - coaster
  - construction
dependencies: []
priority: high
ordinal: 86000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-03: hard to get a coaster to finish and connect. Add very short straights and small height steps, and S-bends (lateral shift while keeping heading) so the end of the track can line up with the station. A closure aid (show the remaining gap, or an automatic connect-to-station) is a separate decision.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03 night: track ends sit on a half-tile grid (plain straight is 4 tiles, offsets 6 tiles), so there are no fine corrections today. Declined a proposal to make the grid finer for quarter-tile pieces; adding half-tile and one-tile straights, small height steps and a full-tile S-bend on the existing grid instead. Codex job task-mw1nrtrw-ku5b6h in worktree theme-park-sim-worktrees/closing-pieces (branch closing-pieces).

2026-11-03 night: merged 0fe14a4. Gentle straights are one tile, not half a tile, because half a tile at the gentle gradient would rise half a height step. Open question for the owner: the full-tile offset is six tiles long like the half-tile offsets; a shorter, sharper S-bend would help tight corrections.
<!-- SECTION:NOTES:END -->
