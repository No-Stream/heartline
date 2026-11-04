---
id: TASK-113
title: Implement the explainable coaster scoring model
status: To Do
assignee: []
created_date: '2026-11-04 22:18'
labels: []
dependencies:
  - TASK-104
priority: high
ordinal: 113000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-04-coaster-scoring.md (eight work items, one agent each). Excitement, intensity and nausea on a soft 0-10 scale built from explainable contributions (airtime first; diminishing returns for repeated inversions; pacing, whip, theming, speed, ride time), ASTM F2291 as a soft upper bound with located explanations in the test report, and guests reading the real ratings (today the coaster facility's intensity is hard-coded to 50 in crates/sim/src/facilities.rs, so guests ignore a coaster's actual rating). Starts after the statistics job (TASK-104) lands.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Scores are sums of named contributions shown in the test report
- [ ] #2 ASTM envelope exceedance lowers scores progressively and the report says where
- [ ] #3 Calibration test on the game's circuits and synthetic layouts within agreed bands
- [ ] #4 Guests choose and react to rides from the real ratings
<!-- AC:END -->
