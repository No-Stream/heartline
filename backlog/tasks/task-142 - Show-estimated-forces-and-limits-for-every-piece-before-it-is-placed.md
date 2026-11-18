---
id: TASK-142
title: Show estimated forces and limits for every piece before it is placed
status: To Do
assignee: []
created_date: '2026-11-18 12:49'
updated_date: '2026-11-18 17:37'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 141000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: the force estimate and limits only show after placing. Today construction.ts:631 shows the candidate force preview only for speed-matched elements and turns (and hills have their own readout). Show the predicted forces against the model's limits for whichever piece or element is selected, before placing, including drops, slopes and hills.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18 (TASK-140 demo build): concrete cases where the missing readout hurts. TerrainRise placed at 23.75 m/s gave -2.48 g / 4.04 g and 1.82 g longitudinal, LevelToGentleUp and GentleUpToLevel at 30 m/s gave 5.99 g and -3.30 g; none showed any warning before placing. Slope transitions, TerrainRise and TerrainFall have no readout at all.
<!-- SECTION:NOTES:END -->
