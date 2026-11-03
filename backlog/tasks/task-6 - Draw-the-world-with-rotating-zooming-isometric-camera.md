---
id: TASK-6
title: 'Draw the world with rotating, zooming isometric camera'
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 03:24'
labels:
  - rendering
  - world
milestone: m-1
dependencies:
  - TASK-3
  - TASK-5
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Draw terrain and paths in the chosen renderer with four rotated views and smooth zoom. Input goes through an action layer so that touch can be added later. Design sections 7 and 9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All four views and smooth zoom work on a 512 by 512 map at the target frame rate
- [x] #2 Picking correctly identifies tiles and objects in every view
- [x] #3 Input code uses abstract actions and does not assume a mouse
- [x] #4 Screenshot tests cover each view
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
World draws with water, cliffs, paths and the underground entrance in four rotations with zoom and ID-buffer picking; input goes through abstract actions. Hardware frame rate on a 512x512 map is not yet measured: headless software rendering gave 62 ms per frame. The full-scale performance ticket covers the hardware check.
<!-- SECTION:FINAL_SUMMARY:END -->
