---
id: TASK-121
title: Bring cold corkscrew validation under 5 ms
status: Done
assignee: []
created_date: '2026-11-05 04:22'
updated_date: '2026-11-05 07:07'
labels:
  - construction
  - performance
dependencies: []
priority: low
ordinal: 121000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
make bench-construction measures cold per-piece validate() at about 10 ms for Corkscrew and CorkscrewRight against the 5 ms target in the force-limited inversions plan. Choices and element choices are within budget, so this matters mainly for TASK-105 (live g-force overlay while building).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Cold validate() of Corkscrew fell from 13.9 to 3.9 ms and CorkscrewRight from 20.2 to 4.5 ms by caching tangent values at the helix interpolation knots and reusing per-sample kinematics; formulas unchanged, support snapshot identical. Other rolls also roughly halved. Single benchmark readings on a loaded machine vary by a few ms per piece.
<!-- SECTION:FINAL_SUMMARY:END -->
