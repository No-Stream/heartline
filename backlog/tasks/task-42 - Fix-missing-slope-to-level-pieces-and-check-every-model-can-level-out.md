---
id: TASK-42
title: Fix missing slope-to-level pieces and check every model can level out
status: In Progress
assignee: []
created_date: '2026-11-03 16:34'
labels:
  - coaster
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: on the Intamin-inspired Velocity Viper there is no obvious way to return from a slope to level. Its explicit piece list probably lacks the slope-to-level transitions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Velocity Viper offers slope-to-level transitions from every slope it can reach
- [ ] #2 A test for every model checks that from every reachable track end state (slope, bank, curvature) a sequence of allowed pieces returns to straight and level
- [ ] #3 The builder shows the transition pieces under Slopes
<!-- AC:END -->
