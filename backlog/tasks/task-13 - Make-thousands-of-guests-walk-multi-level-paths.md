---
id: TASK-13
title: Make thousands of guests walk multi-level paths
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 01:52'
labels:
  - guests
  - simulation
  - performance
milestone: m-1
dependencies:
  - TASK-5
  - TASK-6
  - TASK-12
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Shared flow-field pathfinding across levels, loose avoidance and group walking, drawn with walking animation. Guests mill around like people and never hop. Design section 6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Benchmark with 10,000 guests including avoidance runs natively and in the browser, and results are recorded
- [ ] #2 Results are identical at any thread count
- [ ] #3 Guests with no route, or whose path is deleted, behave sensibly and never freeze
- [ ] #4 Groups visibly walk together, and crowds look natural
<!-- AC:END -->
