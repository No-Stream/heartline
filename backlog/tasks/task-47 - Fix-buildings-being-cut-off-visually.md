---
id: TASK-47
title: Fix buildings being cut off visually
status: To Do
assignee: []
created_date: '2026-11-03 16:40'
labels:
  - rendering
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 47000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: buildings are drawn cut off. Possible causes to check: sprite depth for multi-tile buildings, atlas frame bounds or anchors, or clipping by neighbouring tiles' depth.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All buildings and flat rides draw completely in all four rotations next to paths, terrain and other buildings
- [ ] #2 Screenshot tests of each building in each rotation
<!-- AC:END -->
