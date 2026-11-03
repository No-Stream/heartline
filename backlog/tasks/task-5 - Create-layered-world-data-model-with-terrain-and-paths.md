---
id: TASK-5
title: Create layered world data model with terrain and paths
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
labels:
  - world
  - simulation
milestone: m-1
dependencies:
  - TASK-2
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Terrain is layered from the start because guests can go underground. Sculpting tools and caverns come later, but the model and multi-level paths are needed before guests. Design section 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Model stores surface terrain, water, carved spaces and path tiles on multiple levels, on maps of at least 512 by 512 tiles
- [ ] #2 Tests cover connecting paths across levels and memory use at the largest map size
- [ ] #3 Deleting terrain or a path under guests or objects leaves the world in a valid state
<!-- AC:END -->
