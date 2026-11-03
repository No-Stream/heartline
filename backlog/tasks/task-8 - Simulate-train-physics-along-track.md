---
id: TASK-8
title: Simulate train physics along track
status: To Do
assignee: []
created_date: '2026-11-03 01:52'
labels:
  - coaster
  - simulation
milestone: m-1
dependencies:
  - TASK-7
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Real forces at a fixed step: gravity, rolling and air drag, lifts, launches and brakes, with g-forces, speed and airtime recorded. Design section 4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tests confirm energy conservation with drag off and sensible values against known ride figures
- [ ] #2 A train completes a test circuit deterministically
- [ ] #3 A stalled train rolls back and is recovered without penalty, and trains never crash
<!-- AC:END -->
