---
id: TASK-157
title: Cut shaped hill confirm time to under 250 ms
status: Done
assignee: []
created_date: '2026-11-20 08:07'
updated_date: '2026-11-22 16:59'
labels:
  - shaped-elements
dependencies: []
ordinal: 154000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Cold Node/WASM bench on shaped-elements (2026-11-20): fully banked turns confirm in 25-54 ms p95, but shaped hills take 200-250 ms (4 cars), 230-485 ms (6 cars) and 320-804 ms (8 cars, worst at 34-36 m/s). The steps track the number of simulator previews in the secant (each about 100-200 ms for long trains on tall hills). Today's classic placement p95 is 164-191 ms. Options: a better analytic first guess (fit a per-model correction from the bench data) so most hills need one or two previews; a shorter preview run-out; showing a 'shaping' state while confirming. Browse latency (one preview) was not measured separately; measure it first.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 make bench-shaped-elements-cold passes with worst-case cold Node/WASM p95 <= 250 ms across all 45 hyper cases
- [x] #2 Browse (solve_preview) and confirm latencies measured and reported
- [x] #3 Existing Horizon Hawk and Daisy Dash saved geometry digests remain green
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Cut cold Node/WASM shaped hill confirm p95 from 320-804 ms down to 173.3 ms worst-case across all 45 hyper cases (overall median 55.9 ms, overall p95 107.2 ms, max 218.1 ms across 1,350 samples) with zero geometry digest drift on Horizon Hawk and Daisy Dash. Optimizations: replaced O(cars^2) minimum_mean_height_for_train with a 2*cars-1 RailHeightTable + monotonic hint cursor, precomputed fixed-point angle/trig schedules (skipping powf when ratio >= 1), reused end_speed_squared between consecutive integration steps, added SolvedProfile::segment_cache (OnceLock) and Piece::sample_for_physics / Piece::heart_at to eliminate redundant trig and curvature calculations during physics calibration, skipped reflect/snap/clone during the 8-step flank height search, verified prefix entry speed once per solve, and avoided double-building circuits in PreviewCache::preview_candidate. Single-preview browse (one calibration + one profile synthesis) measures ~24 ms calibration + ~25 ms synthesis (~50 ms total on 8-car 34-36 m/s hills).
<!-- SECTION:FINAL_SUMMARY:END -->
