---
id: TASK-65
title: Add the Timber Tumbler player journey
status: In Progress
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 23:54'
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

2026-11-03 later: the startup readiness timeout has a likely cause, fixed in 0356dd8: parkBounds threw on a new park before its first facilities message and killed the render loop. Watch whether journey startup is now reliable before changing the readiness wait.

2026-11-03: Incomplete. Scripted the full Timber Tumbler loop, including a shared chain-directive circuit, UI ride test and report checks, station access, prices, queue, opening, revenue, rider selection, memory and confetti assertions. The station is 4 m above ground; one extra chain climb and one return descent make the circuit close. Focused Rust closure test and TypeScript typecheck pass; a separate scratch physics test completed a lap using the game physics configuration. Live diagnostics reached a completed report with ratings and no rollback, opened the ride, observed a queued guest board, and verified coaster income and park revenue increased. Rider selection remains red: near-position scans failed twice at 5.2 min; rendered-bounds scanning timed out after 3000 ticks at 6.9 min; recentering then failed after 5.5 min because the selected rider had exited. No product bug is established, so no proposed bug ticket or expected-failure annotation was added. The 180-second test limit is restored; no complete run or three consecutive runs under three minutes were achieved. Follow-up: instrument rendered rider bounds, pick results, and clicked IDs across frames while following a queued rider; determine whether occlusion or input timing prevents selection, then validate guest memory and crest confetti. Profile construction, terrain targeting, and tick waits to reduce total live runtime, and run the finished journey three times under three minutes.
<!-- SECTION:NOTES:END -->
