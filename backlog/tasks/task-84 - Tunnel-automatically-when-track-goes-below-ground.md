---
id: TASK-84
title: Tunnel automatically when track goes below ground
status: In Progress
assignee: []
created_date: '2026-11-04 00:28'
updated_date: '2026-11-04 17:40'
labels:
  - coaster
  - world
dependencies: []
priority: high
ordinal: 84000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-03, lower priority: when a coaster piece passes through terrain, carve a tunnel automatically (with an opt-out), so going below ground level is easy. The terrain carving and underground view already exist (TASK-18); construction currently refuses terrain clearance.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Placing track below the terrain surface carves a tunnel instead of being refused, with tunnel portals where the track enters and leaves the ground
- [ ] #2 Undo and delete restore the terrain
- [ ] #3 A browser test builds a coaster section below ground through the visible UI
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner raised again on 2026-11-04 after trying to build below ground in a playtest: going below ground must carve a tunnel automatically. Start as soon as a job slot frees.
<!-- SECTION:NOTES:END -->
