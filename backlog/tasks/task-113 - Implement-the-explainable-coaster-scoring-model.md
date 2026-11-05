---
id: TASK-113
title: Implement the explainable coaster scoring model
status: To Do
assignee: []
created_date: '2026-11-04 22:18'
updated_date: '2026-11-05 05:41'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner decisions 2026-11-04: scores may exceed 10, but 10 should mean very high or excellent (calibrate so only outstanding rides reach it); the nausea tolerance rule is loose and soft (nausea above tolerance lowers willingness gradually, no hard refusal); theming counts all scenery equally for now.

2026-11-04 owner: rate every force moment as weak / good / great / too much, not just the g value. Airtime example (vertical g at the heart): about 0 g weak ('an airtime hill offering 0 g is a little sad'); -0.3 to -0.8 g good (floater); -1.0 to -1.3 g great ('-1 g or a hair more is great'); past the model limit (about -1.5 g) too much; about -2 g dangerous. Same idea for positive vertical and lateral. Use the bands in scoring (great airtime raises excitement, too much raises intensity and nausea), in the building overlay labels (TASK-105, bands table in TypeScript), and as the airtime hill's default targets (TASK-101).
<!-- SECTION:NOTES:END -->
