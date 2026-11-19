---
id: TASK-144
title: Large longitudinal force on a straight pullout at the bottom of a steep drop
status: To Do
assignee: []
created_date: '2026-11-18 12:57'
updated_date: '2026-11-18 17:37'
labels:
  - physics
dependencies: []
priority: medium
ordinal: 143000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-18: the live builder estimate shows -1.6 g longitudinal (too much longitudinal) on a straight, unbanked piece at the bottom of a steep drop at 76 mph, beside a 6.6 g vertical pullout. A straight non-turning, non-inverting piece should feel near-zero lateral force and only small longitudinal force (drag, friction, the train being pushed or held by cars still on the slope). Diagnose which term produces it and fix if it is a bug.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-19 (dt-estimate): The six ride files pass both finished-track front-car and worst-car comparisons at 0.2 g per peak and 0.5 m/s entry speed. The informational placement report has two downstream-track failure messages and one estimate-error message. The latter is Timber Tumbler's steepening descent: the level virtual run-out changes train-averaged gravity while the train straddles the steep-to-level join, yielding 0.656 g longitudinal versus 0.304 g on the finished track. The owner's exact geometry is still unavailable, so this does not reproduce the reported pullout anomaly. See TASK-146 for ride-level counts.

2026-11-19 (dt-estimate, partial): The ignored longitudinal-diag test and its print helper are dropped; they were exploratory and their 0.63 g result is recorded below. The owner's exact geometry remains unavailable, so that observation is not a reproduction. In the corrected same-start open-versus-closed demo comparison, the maximum sampled longitudinal difference is 0.0 g and the earlier 0.4 g bound is restored to 0.3 g. Placement previews still have unresolved differences on final pieces because later track is unknown; see TASK-146.

2026-11-18 diagnosis (branch longitudinal-diag, ignored test diagnose_steep_drop_pullout_longitudinal_forces in crates/sim/src/physics/tests.rs): a catalog 60-degree drop into a level straight pullout at 34 m/s measures 6.17 g vertical, 0.000 g lateral and at most 0.63 g longitudinal at 200 Hz, so the measured path does not produce -1.6 g. Heart-line offset and train coupling contribute real but small longitudinal force. The builder estimate steps at 50 ms and interpolates acceleration when correcting sample forces (physics/preview.rs:111, sim-wasm construction.rs:483), which is the leading suspect for the larger estimated values. Needs the owner's exact track; continue under TASK-146.

Diagnostic branch longitudinal-diag is not merged; merge the ignored test with the TASK-146 fix or drop it then.

2026-11-18 (TASK-140 demo build): on a 45-degree drop pullout at 25 m/s the 200 Hz run also measures 1.12 g longitudinal (front car) and -0.93 g on the back car, matching the estimate, and large banked speed turns on level track read 0.7-0.8 g longitudinal in both the estimate and the test. So part of the longitudinal force is in the measured path, not only in the estimate. Not diagnosed further.
<!-- SECTION:NOTES:END -->
