---
id: TASK-47
title: Fix buildings being cut off visually
status: Done
assignee: []
created_date: '2026-11-03 16:40'
updated_date: '2026-11-03 18:12'
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
- [x] #1 All buildings and flat rides draw completely in all four rotations next to paths, terrain and other buildings
- [x] #2 Screenshot tests of each building in each rotation
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Facility sprite depth increases toward the top edge. The real-renderer fixture covers all seven facility kinds in four camera rotations with paths, raised terrain, and neighboring buildings; a separate scene test checks the depth gradient. The facility fixture passed 28 cases and the scene test file passed 35 tests.
<!-- SECTION:NOTES:END -->
