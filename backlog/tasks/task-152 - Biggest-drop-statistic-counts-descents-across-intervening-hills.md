---
id: TASK-152
title: Biggest drop statistic counts descents across intervening hills
status: To Do
assignee: []
created_date: '2026-11-18 17:36'
labels:
  - coaster
  - physics
dependencies: []
priority: medium
ordinal: 149000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while building the new demo (TASK-140). largest_downhill_drop (crates/sim/src/physics/testing.rs:1588) returns the largest fall from any earlier high point to any later low point, ignoring climbs in between. The new demo's main drop is 34 m (60 m crest to 26 m), but the ride info shows Biggest drop 40 m, because the track later returns to the 20 m station through several hills and climbs. Scoring uses this value (excitement term biggestDrop). Enthusiast usage and RCDB measure one continuous descent. Fix: reset the high point at each local minimum followed by a climb (with a small tolerance for noise), and add a test with a drop, a climb and a lower finish.
<!-- SECTION:DESCRIPTION:END -->
