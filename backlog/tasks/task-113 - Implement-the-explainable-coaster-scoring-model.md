---
id: TASK-113
title: Implement the explainable coaster scoring model
status: Done
assignee: []
created_date: '2026-11-04 22:18'
updated_date: '2026-11-05 23:14'
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

2026-11-05 owner approved the calibration proposals (floor excitement at 0; Arrow looper excitement about 4-5; drop the hyper and accelerator nausea targets; rebuild demo and player circuits as good rides in a separate ticket; journeys check honest reports instead of excitement minimums) and added: scores should follow enthusiast preferences: RMC and Intamin Blitz much higher than an Arrow and quite a bit higher than a traditional B&M.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Explainable coaster scoring is on master: per-car heartline features, model force bands matching the overlay's FORCE_RATING_BANDS, an ASTM F2291 comfort envelope as a soft bound with report notes, contributions per score that name the force axis, value, limit and duration, track theming, guests reading the real ratings (soft nausea tolerance), and whole-train statistics (airtime, forces, speed and layout) in the testing panel. Calibrated to the owner's enthusiast ordering: Intamin Blitz 11.1, RMC 10.8, B&M hyper 6.6, accelerator 5.8, Arrow looper 4.1, family wooden 3.0; excitement floored at 0. The game's own circuits score honestly low until rebuilt (TASK-101 task 7); the panel's wording is made concise in TASK-130. Gates on merged master: sim lib, circuit and statistics tests, wasm 143, report types, full feature 187/187, journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
