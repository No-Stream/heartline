---
id: TASK-16
title: Add player-built queues and coaster boarding
status: In Progress
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 08:37'
labels:
  - guests
  - coaster
milestone: m-1
dependencies:
  - TASK-10
  - TASK-14
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 16000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Queues are built from path pieces, as in RCT2. Riders feel the physics, which feeds their enjoyment, nausea and memories. Design sections 4 and 6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Guests queue, board, ride and exit a player-built coaster in a browser test
- [ ] #2 Forces recorded on the ride change rider nausea, enjoyment and memory
- [ ] #3 Queue behavior holds up when a coaster is edited or deleted
- [ ] #4 Deleting track with a train on it recovers riders to the station (moved here from the building tool, since riders exist only once boarding does)
- [ ] #5 Clicking a guest who is riding a coaster opens their details panel
<!-- AC:END -->
