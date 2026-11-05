---
id: TASK-119
title: Fix a 286 g longitudinal force spike in ride tests
status: Done
assignee: []
created_date: '2026-11-05 00:57'
updated_date: '2026-11-05 01:34'
labels: []
dependencies: []
priority: high
ordinal: 119000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found 2026-11-04 by the ride statistics work: a Velocity Viper test ride reports a peak longitudinal force of 285.9 g. Brakes decelerate at 3 m/s^2, so the spike comes from a speed set directly or an acceleration computed across a discontinuity (station, rollback recovery, launch, lap wrap).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Root cause found and fixed
- [x] #2 Regression test bounds longitudinal readings on all journey circuits and the demo
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Cause: entering a chain lift set speed directly to lift speed (about 136 g when a launch fed a lift). Lifts now pull a slow train up gradually and let a fast one coast until the chain catches it; a test bounds longitudinal readings at 2.5 g on the journey circuits and demo.
<!-- SECTION:NOTES:END -->
