---
id: TASK-31
title: Keep curvature continuous between track pieces
status: Done
assignee: []
created_date: '2026-11-03 02:43'
updated_date: '2026-11-03 02:56'
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
- [x] #1 Consecutive pieces in the same turn keep continuous curvature, and the connection state carries what is needed for that
- [x] #2 A physics test on back-to-back turns shows no lateral g dip at joins
- [x] #3 Straight-to-turn transitions still ease in smoothly
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Connector state carries horizontal and vertical curvature; seventh-degree Hermite pieces for entry, continuation and exit. Lateral g at joins on back-to-back turns at 20 m/s went from 0.0001 g to 1.274 g, against 1.270 g mid-turn.
<!-- SECTION:FINAL_SUMMARY:END -->
