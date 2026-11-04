---
id: TASK-54
title: Move the park tick and cross-object rules into a Park type
status: Done
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-04 00:17'
labels:
  - refactor
  - simulation
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-simulation-code.md
priority: high
ordinal: 54000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Simulation review steps 5 and 6: WasmSimulation owns the whole tick and every rule involving more than one kind of object, and the thread-count determinism test checks a hand-made copy of the tick. Commands return booleans and errors become strings.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 sim::park::Park owns state, tick_with and state_hash; the bridge delegates to it; the determinism test runs Park::tick_with at 1, 2 and 4 threads; commands return typed errors mapped once in the bridge; getters do not mutate
- [x] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Closed 2026-11-03: make ci at 9987c3e passed everything except the piece-catalog baselines (expected after the lift piece removal), refreshed in 8be816e.
<!-- SECTION:NOTES:END -->
