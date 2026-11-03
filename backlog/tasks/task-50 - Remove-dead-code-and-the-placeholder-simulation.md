---
id: TASK-50
title: Remove dead code and the placeholder simulation
status: In Progress
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-03 16:47'
labels:
  - refactor
  - chore
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-browser-code.md
priority: high
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser review finding 6 and simulation review step 1: the unused renderer.ts and guest-panel index re-export, block-marker drag actions that are sent but never handled, the coaster model choice passed as a fake piece id, and the placeholder Agent and Simulation in crates/sim/src/lib.rs whose threaded test exercises the placeholder instead of guests. Six bridge exports are never called.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Dead code is removed, the coaster model choice has its own action, Executor lives in its own module and tick_bench uses real guests
- [ ] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->
