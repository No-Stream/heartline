---
id: TASK-33
title: Tidy guest modules and export theme styles properly
status: In Progress
assignee: []
created_date: '2026-11-03 04:07'
updated_date: '2026-11-03 04:08'
labels:
  - simulation
  - art
  - chore
milestone: m-1
dependencies: []
priority: medium
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parallel work left guest code spread across guests.rs, guests/walking.rs, walking.rs, walking_tests.rs, guest_behavior.rs, park_guests.rs and visits.rs, and the coaster colours reach the renderer as metadata on one grass drawing. Both work but will confuse later work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Guest code lives in one guests module directory with clear submodules, with no behaviour change and all tests passing
- [ ] #2 Theme styles that the renderer needs (track colours, ink, line weight) are exported once per theme in atlas.json, not attached to a sprite
<!-- AC:END -->
