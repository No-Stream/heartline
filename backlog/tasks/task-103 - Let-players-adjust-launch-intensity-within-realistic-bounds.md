---
id: TASK-103
title: Let players adjust launch intensity within realistic bounds
status: In Progress
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-04 18:34'
labels: []
dependencies: []
priority: high
ordinal: 103000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner playtest 2026-11-04: a launched coaster rolled back and there was no way to fix it except rebuilding. Launch pieces should have an adjustable intensity (target exit speed or acceleration) within realistic per-model bounds, set in the builder for each launch, saved with the coaster and shown in the test report.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each launch has an adjustable intensity within per-model realistic bounds
- [ ] #2 Raising intensity fixes a rollback in a test ride
- [ ] #3 Covered by a browser test
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner correction 2026-11-04: launch limits should come from what the launch technology can physically achieve (magnetic LSM/LIM and hydraulic: realistic peak acceleration and top speed), not from a per-model cap. A future Mack-style hyper must not be limited to a weak manufacturer launch. Apply as a follow-up to the running job.
<!-- SECTION:NOTES:END -->
