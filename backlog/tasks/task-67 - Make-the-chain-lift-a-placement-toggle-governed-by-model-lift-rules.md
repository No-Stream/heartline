---
id: TASK-67
title: Make the chain lift a placement toggle governed by model lift rules
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 21:43'
labels:
  - coaster
dependencies: []
priority: high
ordinal: 67000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 5. chain builder mode, LiftRules, ChainNotAllowed, remove the four lift piece ids.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented chain builder mode, model LiftRules, ChainNotAllowed, wasm/worker command and H binding; converted former lift circuits to base pieces with chain enabled. UpSixty represents the 60-degree model limits; 60-degree geometry remains TASK-69. Curved climbing lift assertion remains ignored for TASK-68; the existing 60-degree assertion remains ignored for TASK-69. Only ordinary track can carry a chain; stations, brakes, launches and other special pieces ignore the toggle and retain their kind.

Validation: sim crate 384 passed across unit and integration tests, 3 ignored; sim-wasm 91 passed; npm test 199 passed; typecheck, formatting, and make build passed. Construction, coaster boarding, and both block-section browser tests passed. The testing overlay baseline was refreshed: the complete report at f4044c4 matches the chain-toggle circuit exactly. The earlier f4044c4 proximity sampling arithmetic fix changed which support samples win for two near-miss markers, predating this migration. In the prior browser run, wooden-twister passed on focused rerun after waiting for station readiness.
<!-- SECTION:NOTES:END -->
