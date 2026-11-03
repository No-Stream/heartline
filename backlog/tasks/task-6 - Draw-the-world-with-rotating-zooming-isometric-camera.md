---
id: TASK-6
title: 'Draw the world with rotating, zooming isometric camera'
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
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
- [ ] #2 Picking correctly identifies tiles and objects in every view
- [ ] #3 Input code uses abstract actions and does not assume a mouse
- [ ] #4 Screenshot tests cover each view
<!-- AC:END -->
