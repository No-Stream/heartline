---
id: TASK-63
title: Add a Rust builder flexibility test over the real construction API
status: To Do
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 20:10'
labels:
  - test
  - coaster
dependencies: []
priority: high
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 1. Connector exhaustion with per-model capability table, seeded random walks across worlds and station heights, closure; fails today on climbing turns, curved and 60-degree lifts.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Connector exhaustion is implemented; random walks and A* closure remain incomplete. An exploratory open-world walk reached no continuation; investigation and a reproducible ticket are still needed. Demo station at absolute 4 m intersects terrain at the sampled location. The full eight-walk matrix has not been shown to fit the five-second budget.

Exploratory reproduction: Timber Tumbler, flat 128×128 world, start (50, 50, z=2), seed 0; no station was placed. Prefix: [LiftFlat, LiftFlat, LiftFlat, LiftFlat, LiftFlat, LiftFlat, LiftLevelToGentleUp, LiftGentleUpToLevel, LiftFlat, Corkscrew, LiftLevelToGentleUp, GentleUpToLevel, LiftLevelToGentleUp, GentleUpToLevel, Corkscrew, LiftLevelToGentleUp, GentleUp, LiftGentleUpToLevel, LiftLevelToGentleUp, LiftGentleUpToLevel, TurnLargeRightStandalone, AirtimeHill, HeartlineRollRight]. The next choice had no valid continuation. This needs investigation with a station before classifying it as a bug. The connector test passes in 1.32 s; all four ignored capability tests fail as expected. No acceptance criteria are defined to tick.
<!-- SECTION:NOTES:END -->
