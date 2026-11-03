---
id: TASK-64
title: Set up live-mode player journeys and a real game-speed control
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 20:57'
labels:
  - test
  - interface
dependencies: []
priority: high
ordinal: 64000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 2. Worker init with seed and park, ?park=new, a player-facing game-speed control in the Watch bar used by tests too, shared player driver, Playwright projects, make journeys.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented live new/demo park initialization with a seeded worker init message and a truly optional demo coaster. Added the Watch game-speed control (V, ×1/2/4/8/16), speed-aware render clock, tick-count hook, player driver/state and shared Timber Tumbler circuit, Playwright projects, and make journeys. The food-stall smoke passed three serial runs in 14.5 s, 14.6 s, and 14.5 s; direct worker/track browser regressions passed 12 tests. Focused park, WASM preset, circuit, speed Vitest, production build and typecheck checks passed. The chain-toggle driver helper depends on the future chain-toggle implementation. No make ci or commit was run.

The shared driver also covers current station placement, piece steppers, placing, closing, testing, opening, and queue paths. Focused browser checks passed for live station placement and construction controls; the future chain toggle remains deferred.
<!-- SECTION:NOTES:END -->
