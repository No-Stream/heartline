---
id: TASK-109
title: Build a fuller starter park as a realistic demo
status: To Do
assignee: []
created_date: '2026-11-04 17:24'
labels: []
dependencies:
  - TASK-95
  - TASK-101
  - TASK-104
  - TASK-106
  - TASK-107
priority: medium
ordinal: 109000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04, to start once the current arcs land (force limits TASK-95, builder panel TASK-106, Finish coaster TASK-107, RMC pieces TASK-101, stats TASK-104, sound TASK-28). An Opus agent designs and builds a more ambitious starter park: one well-designed coaster of each model (Timber Tumbler, Velocity Viper, Briar Blitz, plus any new model such as TASK-99 if it has landed) with realistic layouts that use the new elements and ride within the force limits, several flat rides, themed areas with paths, queues, scenery, lamps, terrain and water, and facilities placed sensibly. It doubles as a realistic test: the park must load and run with guests riding everything, and a browser journey should check that every ride runs and guests use it.

Note 2026-11-06: a children's coaster now exists. Daisy Dash by Windmill Ride Works (model key `family_coaster`) takes riders from 100 cm, so the starter park can give children and cautious guests a coaster; its reference circuit is `[Daisy Dash]` in `web/e2e/player/circuits.txt`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One fleshed-out coaster per model with good ratings and no rollbacks
- [ ] #2 Several flat rides, themed areas, scenery and facilities
- [ ] #3 A journey checks every ride runs and guests ride it
<!-- AC:END -->
