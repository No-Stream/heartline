---
id: TASK-65
title: Add the Timber Tumbler player journey
status: In Progress
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 21:43'
labels:
  - test
dependencies: []
priority: high
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 3. Full gameplay loop with real input; fails today at the curved-lift step.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-03: the journey runs through path, food, drink, restroom, model choice, station and chain climb, then fails as expected (test.fail) with 'no climbing turn offered while the chain lift climbs' (29 s). Still to write once TASK-67/68/69 land: the rest of the circuit, testing report, prices, opening, riders paying, guest memory and confetti; and the circuit in circuits.txt with chain directives the Rust reader understands.

2026-11-03: the journey now turns the chain on with H (asserted via data-chain) and climbs with plain climb pieces. Fragility to fix: in live mode the journey waits for gameTest.ready, which flips while the game runs; one run timed out waiting for it at startup and the next two passed in 30 s. Prefer waiting on a monotonic readiness signal such as the first rendered tick.
<!-- SECTION:NOTES:END -->
