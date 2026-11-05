---
id: TASK-119
title: Fix a 286 g longitudinal force spike in ride tests
status: To Do
assignee: []
created_date: '2026-11-05 00:57'
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
- [ ] #1 Root cause found and fixed
- [ ] #2 Regression test bounds longitudinal readings on all journey circuits and the demo
<!-- AC:END -->
