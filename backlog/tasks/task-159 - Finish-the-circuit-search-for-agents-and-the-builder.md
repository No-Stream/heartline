---
id: TASK-159
title: Finish the circuit search for agents and the builder
status: In Progress
assignee: []
created_date: '2026-11-20 16:45'
labels:
  - construction
  - tooling
dependencies: []
priority: high
ordinal: 156000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-20 ("a goal both for humans and agents"): from wherever the track ends, find short piece sequences that bring the train back into the station with correct position, height and heading, the train reaching the station, every new piece inside its model's force limits and rated-speed windows, and no collisions. Return a ranked list (pieces, forces and speeds, brake energy) or a clear reason with what would need to change. Agents use `tools/design ride <file> finish`; players use the builder's Finish coaster action. Both use the same Rust search, which extends the TASK-92 completion search. Plan: docs/superpowers/plans/2026-11-20-finish-circuit.md. Motivated by the Horizon Hawk redesign (TASK-149, TASK-156), where closing the circuit took most of the layout time.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-20: baseline measurements. The old finish search took 10.6 s natively on Horizon Hawk with its last three pieces removed and found nothing (end 57 m away, 90 degree heading mismatch). A full Horizon Hawk lap takes 0.66 s natively, so the new search simulates only the new pieces from the builder preview's snapshot at the open end.

2026-11-20: implemented on branch circuit-finish. `tools/design ride <file> finish` lists up to 5 ranked options (pieces, per-piece speeds and worst-car forces, arrival speed, brake energy, a closed-lap check under default and construction-test physics); `finish apply <n>` places one and refuses an option that fails the check. The builder previews the best option and has Previous/Next option buttons. Search alone, release build, warm preview: Horizon Hawk without its last 3 steps 35 ms (two one-tile straights), Velocity Viper without its last 4 steps 183 ms, Timber Tumbler without its last 5 steps 152 ms, Horizon Hawk without its last 4 steps 3.0 s and no safe finish (too fast for classic turns; its real ending uses shaped turns, which the search does not place). Limits: no shaped elements, rolls, lifts or launches as candidates; no station arrival speed cap (the game has none); per-car entry speeds past the open end are taken from front-car samples.

2026-11-20 gates on circuit-finish: make lint, journeys (11 passed) and the @fast browser suite pass; all finish tests pass. Remaining failures all come from the unfinished Horizon Hawk layout in base commit 035f766 (content changed, goldens not updated): design-tools park_report golden, horizon_hawk_compact_recipes_preserve_the_original_shaped_geometry, shaped_piece_prefix_expectations_count_canonical_hill_and_turn_ids, web park-bundle digest test, and the park-description e2e test. They need updating once the Hawk layout is final. Daisy Dash also fails `tools/design check` force headroom on this base (pre-existing).

2026-11-20: merged master 6661fe4 (final Horizon Hawk). The final layout ends in the same place, so the finish results are unchanged: without its last 2 steps one one-tile straight, without its last 3 two one-tile straights (22.5 m/s, ride check pass), without its last 4 or 5 no safe finish (a classic large turn reaches 1.23 g lateral at 22.8 m/s). Without its last step the track already reaches the station, and the search now says to close the circuit instead of reporting no route. The Horizon Hawk failures from the base commit are gone. make lint and make check pass. Daisy Dash fails force headroom on master 6661fe4 too (same values, so not caused by this branch), and `heartline-design check` exits 0 despite those failures.
<!-- SECTION:NOTES:END -->
