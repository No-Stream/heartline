---
id: TASK-146
title: Builder force estimate disagrees with the test run
status: To Do
assignee: []
created_date: '2026-11-18 13:05'
updated_date: '2026-11-18 17:37'
labels:
  - coaster
  - physics
dependencies: []
priority: high
ordinal: 145000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: a layout looked fine in the builder's live force estimate, then the test run showed -2.0 g dangerous airtime and 1.7 g / -1.9 g too much longitudinal on the same stretch. The estimate must predict what the test measures (same cars, same filtering, same speed). Compare estimate and measurement sample by sample on the owner's exported session and fix the source of the difference. Related: TASK-126 (filtered limits), TASK-142, TASK-144 (longitudinal on pullouts).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-18: blocked on the owner's exported session or exact track. TASK-144's diagnostic (branch longitudinal-diag, ignored test diagnose_steep_drop_pullout_longitudinal_forces) found the measured path gives at most 0.63 g longitudinal on a 60-degree drop pullout, so the large longitudinal values come from the estimate. Leading suspect: the estimate steps at 50 ms and interpolates acceleration when correcting sample forces (physics/preview.rs:111, sim-wasm construction.rs:483).

2026-11-18 diagnosis on the new demo layout (TASK-140, branch demo-ride). Scratch comparison: for every piece, the builder estimate (preview_open_track, front car, 50 ms steps) against a 200 Hz run of the same train read for the front car and for every car.

Vertical g per piece (min / max), estimate front car | test front car | test worst car:
- Crest, GentleToSteepDown: 0.30 / 0.89 | 0.30 / 0.89 | -0.24 / 0.89 (back car)
- LevelToGentleDown at the crest: 0.66 | 0.66 | 0.28 (back car)
- SteepToGentleDown: 2.16 | 2.17 | 2.66 (back car)
- Main pullout, GentleDownToLevel: 3.91 | 3.92 | 4.20 (back car); longitudinal min -0.49 | -0.50 | -0.93
- Ejector hill 16x96: -0.82 / 3.42 | -0.82 / 3.43 | -0.82 / 3.43
- Stall: -0.41 / 3.49, lateral 0.66 in all three; longitudinal min -0.74 | -0.82 | -1.04
- Zero-g roll: -0.37 / 3.33 | same | -0.39; longitudinal min -0.86 | -1.05 | -1.19
- Ejector hill 8x64: -1.08 / 3.54 in all three; longitudinal min -0.88 | -0.88 | -1.12
- TerrainFall on the return: 0.73 / 1.28 | 0.71 / 1.29 | 0.59 / 1.33

Conclusions:
1. The 50 ms stepping is not the main source. Vertical and lateral estimates match the 200 Hz front car within 0.02 g on every piece. Longitudinal extremes match within 0.2 g; the largest per-sample difference is 0.36 g (estimate -0.82, run -0.46) at 606 m inside the zero-g roll. Recording each step's mean acceleration at the step midpoint instead of its end did not change that difference, so the half-step lag is not the cause; the remaining longitudinal error inside rolls is unexplained.
2. The large gaps are front car versus worst car. The estimate reads only the front car (step_preview passes reading_count 1), while the test run's statistics, force advice and ride info report the most extreme car. Back cars are whipped over crests and into pullouts faster: on this layout the back car gets -0.24 g over the main crest where the estimate shows +0.30 g, and 4.20 g in the pullout where it shows 3.91 g. The owner's case (estimate fine, test -2.0 g airtime and about 1.8 g longitudinal) fits this: a back car over a crest after a fast drop.
3. The hill readout is a separate, larger error: a point mass at entry speed, so about 0.15-0.4 g weaker airtime than even the front car (TASK-151).
4. Latent bug: TestRunRecorder::record uses super::TIME_STEP (5 ms) for force-band and comfort durations even when the preview feeds it 50 ms readings, so preview durations are 10 times too short. Nothing in the UI reads those preview durations today.

Proposed failing test (crates/sim/src/physics/tests.rs): take the demo prefix through GentleToSteepDown (pieces 0..=8), run preview_open_track, and assert its minimum vertical g on piece 8 is within 0.1 g of the worst-car minimum that ConstructionTestRunRecorder records for piece 8 on the closed demo run. Today: estimate 0.30 g, worst car -0.24 g. Fix direction: step the preview with readings for every car (reading_count = cars), correct each car's sample at its own distance, and report the worst car per sample, matching merge_worst_car_statistics.
<!-- SECTION:NOTES:END -->
