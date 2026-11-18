---
id: TASK-142
title: Show estimated forces and limits for every piece before it is placed
status: Done
assignee: []
created_date: '2026-11-18 12:49'
updated_date: '2026-11-18 19:22'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 141000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: the force estimate and limits only show after placing. Today construction.ts:631 shows the candidate force preview only for speed-matched elements and turns (and hills have their own readout). Show the predicted forces against the model's limits for whichever piece or element is selected, before placing, including drops, slopes and hills.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The worker already simulated the selected candidate on complete construction updates; the panel restricted its candidate readout to speed-matched pieces and turns. It now covers ordinary pieces as well; hills retain their existing hill inspector. Candidate samples are sent only for the selected preview and rendered with the existing limit-closeness colours. The simulation still uses its existing construction caches and does not simulate every available choice.

Validation: `make check`, `make test-web`, and `make lint` passed. The feature suite passed 186 of 190 tests; two marker-position baselines were updated and their focused reruns passed. The dark workshop screenshot and underground cutaway screenshot failed twice and remain open. The underground image includes the new candidate ribbon and a changed existing force annotation (5.8 g to 5.9 g), which needs investigation before updating its baseline. Native placement-update p95 before/after was 133.8/126.4 ms (Timber Tumbler), 122.0/102.6 ms (Velocity Viper), and 240.7/113.1 ms (Briar Blitz); run-to-run contention limits interpretation.

2026-11-18 follow-up after merging current `master`:

- The existing underground force annotation comes from the full construction force preview. Its peak sample at distance 424 is 5.875977738 g, so the existing one-decimal formatter displays 5.9 g. The model's vertical rating boundary is 5.85 g, so `dangerous vertical` is also correct. TASK-142 adds a separate candidate preview and ribbon; it does not alter the full construction force estimator. The prior 5.8 g screenshot text was stale, not a simulation bug.
- The underground candidate ribbon is the intended selected-piece force preview. The dark workshop screenshot captured a compact toolbar (fit level 7); the current 1280 px run selects fit level 1 and displays the full tool names. Both differences are explained and intended, so the baselines were re-recorded.
- Reviewed these regenerated images: `web/e2e/dark-theme.spec.ts-snapshots/dark-coaster-workshop-linux.png`, `web/e2e/underground-cutaway.spec.ts-snapshots/underground-cutaway-coaster-linux.png`, and `web/e2e/underground-cutaway.spec.ts-snapshots/underground-cutaway-coaster-dark-night-linux.png`.
- The requested gates ran once in order. `make check` passed (227 focused Rust tests, 41 fast browser tests, 459 web tests, 9 art tests, and type checks). `make test-rust` passed 895 tests with 3 filtered. `make test-web` passed 459 web tests and 9 art tests. `make lint` passed. `PLAYWRIGHT_PORT=4741 make e2e-feature` had 189 passed, one piece-catalog setup timeout, and three skipped rotations. The runner now creates the fixture through `tools/run-in-budget.sh` before Playwright starts; the focused piece-catalog rerun passed all 4 rotations. `PLAYWRIGHT_PORT=4742 make journeys` had 5 passed and 6 failed because a global `[data-rating]` locator matched both the builder and the ride window. The journey helper now scopes report assertions to `[data-testing]`; a focused Briar Blitz rerun passed. The full gates were not rerun.

2026-11-18: merged to master (cfedbfb); master gates: build, Rust, Vitest, lint, typecheck, feature 195/195, journeys 11/11 after scoping the tunnels journey rating selector.
<!-- SECTION:NOTES:END -->
