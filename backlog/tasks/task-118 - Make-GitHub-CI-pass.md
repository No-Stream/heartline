---
id: TASK-118
title: Make GitHub CI pass
status: To Do
assignee: []
created_date: '2026-11-05 00:27'
updated_date: '2026-11-18 16:45'
labels: []
dependencies: []
priority: high
ordinal: 118000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repo went public as No-Stream/heartline on 2026-11-04. The first CI run (Actions run 41936488470) failed: lint (formatting and Clippy from merged branches; fixed locally in e96260e, not yet pushed) and e2e (12 browser tests on GitHub's runner: timeouts in construction, Immelmann, path panel layout, pause, audio, the operate journey and coaster boarding; screenshot mismatches in dark theme, guest panel and construction; the toolbar fit sweep). The test job passed. The suite was tuned on the owner's machine, so GitHub's slower 4-vCPU software-rendering runner exposes timing assumptions and platform-specific screenshot baselines. Diagnose each failure from the uploaded artifacts: decide per test whether it is a real bug (fix it), a timing assumption (wait on observable state instead of fixed timeouts), or a rendering difference (separate CI baselines generated on the runner, or a tolerance justified per test). Keep the local gate unchanged in strictness.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CI is green on master on GitHub
- [ ] #2 Each failure explained and fixed at its cause
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-05: postcard replaced bincode (RUSTSEC-2025-0141; save version 2, version 1 refused), CI split into smaller jobs, two browser tests use deterministic waits; merged and pushed as 6595b23. GitHub run 42119910401: audit, lint, threaded and flexibility tests, e2e build and smoothness passed; test and the feature and journey e2e jobs were still running at handoff.

2026-11-05: run 42121748977 passed every job except e2e (feature), which hit the 20-minute job limit and was cancelled; the feature project is now sharded into three CI jobs.

2026-11-05: sharded run 42128358630: all jobs green except the feature shards (16 failures: small screenshot differences and runner timeouts). Branch ci-feature investigating (Docker parity likely).

2026-11-06: merged into master (ci-journeys): browser tests run in the pinned Playwright image with npm ci, journeys sharded in the container, world quads drawn as indexed triangles (Docker frames 333 -> 167 ms). Branch gates: Docker journeys 9/9, local journeys 9/9, feature 187/188 before a stale draw-count test fix. Not yet confirmed on GitHub: check the first run after this push.

2026-11-18: the last GitHub runs (2026-11-07) failed in every browser shard with the run-in-budget guard ('Run browser tests with npm run e2e'). Commit 2bde5f5 sets the budget flag inside the Playwright container; it is on master but not pushed, so it is untested on GitHub. Check the first run after the owner pushes.

2026-11-21: Inspected failed runs 44886580652, 44875912291, 44868563243, 44867177899, and 44862887909. Every run failed all three `e2e` journey shards and all six `e2e-feature` shards: the Playwright container's Vite config calls `git rev-parse HEAD`, and Git rejects the checkout at `/__w/heartline/heartline` because its owner differs from the container user. Added a global safe-directory entry after checkout in both container jobs. Runs 44875912291 and 44862887909 also failed `test` / `make test-rust` on `classic_builder_outputs_keep_the_recorded_digests`, `content_ride_builder_hashes_match_recorded_baselines`, and `content_ride_builder_output_traces_match_recorded_baselines`; all three tests pass on this worktree's base, so their expected digests remain unchanged. No other failing jobs or causes appeared in these five runs. A root Playwright container reproduces the Git failure before the safe-directory command and succeeds afterward.

The first local Docker feature run built successfully but finished 191/200 tests with nine failures: two replay tests could not invoke `tools/design` because the Playwright image has neither `make` nor Rust, and seven visual cases had stale or missing Docker snapshots. The workflow now builds the CLI on Ubuntu 24.04, uploads it, and downloads it into both Playwright jobs; `THEME_PARK_DESIGN_TOOLS_PREBUILT=1` makes `tools/design` use that binary. The two replay tests passed 2/2 in the local Playwright image, and the seven visual cases passed their focused update and no-update runs. After inspecting the report and canvas diffs, I refreshed the affected screenshots without changing thresholds or retries. The final local Docker gates pass: `THEME_PARK_DESIGN_TOOLS_PREBUILT=1 make e2e-feature-docker` passed 200/200 in 32.2 minutes, and `THEME_PARK_DESIGN_TOOLS_PREBUILT=1 make e2e-journey-docker` passed 11/11 in 14.2 minutes. `make lint` and `tools/run-in-budget.sh cargo test --workspace --no-fail-fast -- --skip track::construction::walk_tests::seeded_` also pass.

The requested worktree path is `theme-park-sim-worktrees/ci-green`. Since the `ci-green` branch already existed, this worktree uses `ci-green-task-118` from the master base at `6d3286d`. No push was made; GitHub confirmation remains pending.

2026-11-21: Portable native math work on `portable-math` from master `56fc378`: routed simulation transcendental operations and hypot through a shared `libm` wrapper module, made integer powers deterministic, and enforced the ban on platform-dependent float methods in Clippy. Re-recorded builder digests and six floating-point hill metadata lines; the stored f32 hill samples and ride files did not change. The seven native ride checks passed before and after. Comparing 3,244,082 force fields found a maximum change of 3.17613085832e-8 g, with no changed check outcomes, failure lists, sample identities, or report structure.

Validation on the portable-math branch: full Rust workspace suite passed with `--no-fail-fast` and the requested seeded-walk skip. All 24 actual test binaries passed in the Playwright Noble container with identical passed/failed/ignored/filtered counts; the initial runner also selected the non-test CLI executable, which caused its wrapper to exit nonzero, so the corrected artifact list and all 24 results were verified independently. `make lint` passed again after the final hill metadata and digest refresh; `make check` passed (233 Rust subset, 497 web, 9 art, 3 fast agreement, and 47 browser tests). `tools/design agree` passed 7/7 rides; full browser ride agreement passed 8/8, feature 200/200, and journey 11/11. No browser screenshot baselines were refreshed. One-run `make bench-builder-updates` placement medians were 139.0 to 102.4 ms for Timber Tumbler and 155.7 to 112.3 ms for Briar Blitz; these shared-machine timings are a coarse sanity check.

A separate before/after builder trace audit covered the three changed classic circuits and five changed content rides: 35,643,331 float tokens, zero non-float structural differences, and maximum absolute numeric delta 3.17613085832e-8 (a lateral force). The seven ride reports had 8,328,513 numeric fields, the same maximum difference, and no outcome or structural changes. The 195 changed state hashes are opaque; detailed builder outputs were compared separately.

2026-11-22: CI run 44985081662 (portable math) passed every Rust test it reached, but the test job hit its 20-minute limit. Cause: the libm crate compiled at opt-level 0 in test builds, so every Rust test binary ran 2-2.6x slower than with glibc (theme-park-sim lib 146.8 s -> 383 s in the test job, 276 s -> 733 s in test-threads). Fix: [profile.test.package.libm] opt-level = 3 (one heavy test locally: 152 s before the libm change, 424 s after, 186 s with the override). The test job was already near its limit before the regression (13m16 of Rust tests plus about 2.5 min of setup and test-web), so the web tests now run in their own test-web job and the Rust tests run in three parallel shards (theme-park-sim, theme-park-sim-wasm, everything else) through make test-rust RUST_TEST_SELECTION=..., which now always passes --no-fail-fast. Slowest single tests (over 60 s on CI): incremental_force_preview_matches_fresh_player_circuit_prefixes, seeded_different_circuits_keep_trains_separate_and_replay, every_model_rated_window_endpoint_passes_with_its_default_train_in_each_applicable_context, native_agreement_check_reports_every_ride_and_attributes_each_failure, horizon_hawk_park_report_is_deterministic_and_matches_the_golden.
<!-- SECTION:NOTES:END -->
