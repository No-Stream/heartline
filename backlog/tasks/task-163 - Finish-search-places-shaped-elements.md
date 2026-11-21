---
id: TASK-163
title: Finish search places shaped elements
status: In Progress
assignee: []
created_date: '2026-11-21 06:31'
updated_date: '2026-11-21 21:25'
labels: []
dependencies: []
ordinal: 159000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The finish-the-circuit search (TASK-159, crates/sim/src/track/construction/completion.rs) only places classic pieces and brakes. On Horizon Hawk with four or more pieces removed it reports 'too fast' because classic turns exceed lateral limits at 22 m/s, while shaped turns solved for the entry speed would fit. Let the search place shaped hills and turns for models that support them, solved for the actual entry speed, within its existing time budget, and keep classic-only results unchanged for other models.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-21, branch finish-shaped. "Without its last N steps" below means the last N ride steps removed, the close step included (so 4 steps = the final 36 m banked turn and the two straights after it), as in the TASK-159 notes. Times are the search alone, release build, warm builder preview (`make bench-finish`).

What the search does now. On models that support shaped elements (hyper, 4 to 8 cars), at level, unbanked, straight ends on the grid axes with an estimated speed of 22 to 36 m/s, it also tries fully banked shaped turns with fixed even radii. Each turn is solved with the builder's solver for the simulated speed at its entry (construction-test physics resumed from the open-end snapshot), so the saved spec re-solves to the same geometry digest. Radii run from where the turn would reach 3.5 g (scaled from the automatic size and its 2.9 g target) up to one 2 m step above the automatic size; measured at 22.8 and 23.0 m/s, the solver refuses two steps above. Each depth solves the two turns whose routes rank best across the beam, judging a turn's exit by the classic heuristic or by one more shaped turn right after it. A turn must end at least 8 m from the station entry, and a finish turn must stay at or under 3.5 g vertical and 0.6 g lateral for the worst car. `finish apply` writes each turn as a shaped ride step with intent `turn {hand, Radius r}` and the solved spec, so `tools/design check` (including the saved-shape re-solve) passes. Shaped hills are not offered: they end at their entry height, so they cannot help reach the station height, and at these speeds they are longer than a station approach.

Speed. Classic candidates are now validated only when they make the next beam or reach the station. Results are identical (all finish outputs for Velocity Viper, Timber Tumbler, Briar Blitz, demo and Daisy Dash without their last 2 to 7 steps compared byte for byte) and the classic search is 2 to 6 times faster; most of the time used to go to support re-routing for candidates the beam then dropped.

Horizon Hawk results: without its last 2 steps, one straight (22.5 m/s arrival, 18 ms). Without 3, two straights (22.5 m/s, 22 ms). Without 4, a 36 m right shaped turn and two straights (22.8 m/s entry, worst car 2.68 g vertical and 0.13 g lateral, arrival 22.5 m/s, 298 ms). Without 5, a straight, the same 36 m turn and two straights (2.68 g, 0.13 g, arrival 22.5 m/s, 400 ms). Without 6, no finish (1.2 s): the original five-piece ending (28 m left turn, straight, 36 m right turn, two straights) exists but the two solves per depth do not reach it; three per depth do not either. Before the 8 m rule the search found a 3-piece left-right-straight finish there in 0.47 s, but its last turn then sits under the rear cars of the train waiting in the station and the closed-lap check measured 0.62 g lateral at dispatch, over the 0.6 g target. Without 27 or 28 steps (fast, 105 to 108 m out), no finish in 0.71 and 0.55 s, and the reason now lists the shaped turns the solver refused.

Test changes: the TASK-159 test that Horizon Hawk without its last 4 steps reports "too fast" now finishes with a shaped turn (new tests for 4 and 5 steps check the saved steps re-solve to their digests and that `check` passes); the too-fast explanation and the state test use Horizon Hawk without its last 27 steps instead.

Not done: shaped hills; turns from diagonal headings (the browser's 2 m radius stepper cannot select their 2.8 m grid); a finish that ends with a shaped turn at the station entry (the builder then has no piece selected, which the ride check's builder snapshots cannot record).
Gates on 1eab904: workspace tests (skipping the seeded walk tests) 1139 passed, 0 failed, 1 ignored; `make lint` passed; `make check` passed (Vitest 497, browser 47); `make ride-agreement RIDE=horizon-hawk` 2 passed. Finish apply now also refuses a completion whose piece ids disagree with its geometry, which keeps the existing tampered-completion test meaningful now that apply rebuilds from geometry.

Backlog review 2026-11-21: shaped turns are placed by the finish search on master (812cfb6; gates recorded in 1eab904 and bc12252). Keep open: the ticket explicitly leaves shaped hills, turns from diagonal headings, and finishes that end with a shaped turn at the station entry unresolved.
<!-- SECTION:NOTES:END -->
