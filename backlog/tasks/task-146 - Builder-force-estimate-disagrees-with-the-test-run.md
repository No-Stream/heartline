---
id: TASK-146
title: Builder force estimate disagrees with the test run
status: To Do
assignee: []
created_date: '2026-11-18 13:05'
updated_date: '2026-11-18 16:45'
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
<!-- SECTION:NOTES:END -->
