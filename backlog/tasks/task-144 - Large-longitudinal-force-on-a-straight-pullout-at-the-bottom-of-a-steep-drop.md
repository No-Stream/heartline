---
id: TASK-144
title: Large longitudinal force on a straight pullout at the bottom of a steep drop
status: To Do
assignee: []
created_date: '2026-11-18 12:57'
updated_date: '2026-11-18 16:45'
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
2026-11-18 diagnosis (branch longitudinal-diag, ignored test diagnose_steep_drop_pullout_longitudinal_forces in crates/sim/src/physics/tests.rs): a catalog 60-degree drop into a level straight pullout at 34 m/s measures 6.17 g vertical, 0.000 g lateral and at most 0.63 g longitudinal at 200 Hz, so the measured path does not produce -1.6 g. Heart-line offset and train coupling contribute real but small longitudinal force. The builder estimate steps at 50 ms and interpolates acceleration when correcting sample forces (physics/preview.rs:111, sim-wasm construction.rs:483), which is the leading suspect for the larger estimated values. Needs the owner's exact track; continue under TASK-146.

Diagnostic branch longitudinal-diag is not merged; merge the ignored test with the TASK-146 fix or drop it then.
<!-- SECTION:NOTES:END -->
