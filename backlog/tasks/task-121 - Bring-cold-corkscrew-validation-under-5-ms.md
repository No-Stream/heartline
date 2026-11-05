---
id: TASK-121
title: Bring cold corkscrew validation under 5 ms
status: To Do
assignee: []
created_date: '2026-11-05 04:22'
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
