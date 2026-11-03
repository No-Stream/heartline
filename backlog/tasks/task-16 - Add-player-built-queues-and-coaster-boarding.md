---
id: TASK-16
title: Add player-built queues and coaster boarding
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 10:44'
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
- [x] #1 Guests queue, board, ride and exit a player-built coaster in a browser test
- [x] #2 Forces recorded on the ride change rider nausea, enjoyment and memory
- [x] #3 Queue behavior holds up when a coaster is edited or deleted
- [x] #4 Deleting track with a train on it recovers riders to the station (moved here from the building tool, since riders exist only once boarding does)
- [x] #5 Clicking a guest who is riding a coaster opens their details panel
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Guests queue for coasters, board by group, feel their car's recorded forces and remember the ride; riders are pickable for the panel; track edits recover riders and queues. Paths now store explicit edge connections so queues cannot be bypassed through adjacent exits.
<!-- SECTION:FINAL_SUMMARY:END -->
