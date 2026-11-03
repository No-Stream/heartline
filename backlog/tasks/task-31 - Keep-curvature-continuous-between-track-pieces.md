---
id: TASK-31
title: Keep curvature continuous between track pieces
status: To Do
assignee: []
created_date: '2026-11-03 02:43'
labels:
  - coaster
  - simulation
milestone: m-1
dependencies:
  - TASK-8
priority: high
ordinal: 31000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Physics found that each piece starts and ends at zero curvature, so lateral force drops from 1.47 g mid-turn to 0 g at the join between two turns at 20 m/s. Riders would feel a jolt at every join. Design sections 3 and 4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Consecutive pieces in the same turn keep continuous curvature, and the connection state carries what is needed for that
- [ ] #2 A physics test on back-to-back turns shows no lateral g dip at joins
- [ ] #3 Straight-to-turn transitions still ease in smoothly
<!-- AC:END -->
