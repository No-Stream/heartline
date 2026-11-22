---
id: TASK-167
title: Shaped elements for every coaster model
status: Done
assignee: []
created_date: '2026-11-21 16:05'
updated_date: '2026-11-22 08:07'
labels: []
dependencies: []
ordinal: 161000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-21-global-limits-and-shaped-elements.md (Part B). Per-model ShapedProfile data; solver reads only the profile; rollout hyper (zero diff), Velocity Viper, Timber Tumbler, Daisy Dash, Briar Blitz; finish search for every profiled model. Supersedes the Daisy Dash choice in TASK-161.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Carry-forward from TASK-161: Daisy Dash still needs an all-model profile rollout that addresses its measured force-headroom failures, updates the ride file and digest/golden expectations, and verifies both physics configurations; the local attempts in TASK-161 did not find a viable layout. Carry-forward from the remaining staged scope in TASK-156: preserve turn-feel and vertical-transition controls, add named inversions with peak-force intent, and finish the remaining showcase rides. The current profile, matrix, parity, browser-agreement, and finish-search work should include these items explicitly.

Remaining carry-forward items from TASK-156 were split into dedicated backlog cards: TASK-168 (shaped inversions with peak-force intent), TASK-169 (lateral-rich turn feel and vertical-transition controls), and TASK-170 (refresh remaining showcase rides with shaped elements).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented profile-driven shaped hills and fully banked turns across all five coaster models (`hyper`, `velocity_viper`, `hybrid`, `wooden_twister`, `family_coaster`):
- Added `ShapedProfile` (unhashed, skipped in saves) to `CoasterModel` in `crates/sim/src/track/models.rs`; removed all hardcoded hyper constants and model-key checks from `crates/sim/src/track/shaped.rs` (enforced by `solver_source_names_no_coaster_model_key`) and `crates/sim/src/track/construction/completion.rs`.
- Kept per-model vibe defaults inside `DesignForceTargets` with 0.25 g headroom while allowing players to push up to the global ASTM F2291 comfort envelope (e.g., true ejector on hyper, strong floater/ejector on Daisy Dash, tighter manual turns) with a mild amber feel note (`exceedsModelFeel`) in the builder UI.
- Fixed low-speed turn radius bank scaling (`crates/sim/src/track/curves.rs`), rail-curvature bank softening in `shaped.rs`, and `PreviewCache::preview_candidate` closing-track runout (`crates/sim/src/physics/preview.rs`).
- Redesigned `content/rides/daisy-dash.ride.json` with 5 shaped floater hills, 3 shaped banked turns, its curved chain lift, and `designMargins: true`; passes all expectations and force headroom under both default and construction-test physics (excitement 2.61 / 3.73, peaks +2.46/-0.23 g and +2.52/-0.31 g). Updated Daisy Dash's recorded builder hash (`a7bc9079de11869c`) and output trace digest (`df5864855eff5eab`); Horizon Hawk digests remain unchanged.
- Added 5-model acceptance matrices (`shaped_elements.rs`, 514 total matrix cases), 10-case per-model native-vs-WASM parity (`make check-shaped-turn-arc-parity`), all-model finish-search tests (`completion_tests.rs`), browser E2E coverage across all 5 models and the hyper ejector feel note (`web/e2e/construction.spec.ts`), and updated `docs/designing-rides.md`.
- Verified: `make lint`, `make test-rust`, `make test-web`, `make test-agreement`, `make check-shaped-turn-arc-parity`, `make bench-finish`, Playwright `@fast`, `ride-agreement` (Daisy Dash 25/25 and Horizon Hawk 41/41), `--project journey` (11/11), and `--project feature` (203/203).
<!-- SECTION:FINAL_SUMMARY:END -->
