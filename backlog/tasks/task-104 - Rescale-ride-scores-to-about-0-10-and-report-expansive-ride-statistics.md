---
id: TASK-104
title: Rescale ride scores to about 0-10 and report expansive ride statistics
status: Done
assignee: []
created_date: '2026-11-04 17:00'
updated_date: '2026-11-05 23:14'
labels: []
dependencies:
  - TASK-95
priority: high
ordinal: 104000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04. Scores (excitement, intensity, nausea or whatever the ratings are) should read on a roughly 0-10 scale (not strictly bounded). Add ride statistics to the test report: seconds of ejector airtime (below about -1 g vertical), seconds of floater airtime (about -1 to 0 g), seconds of strong positives, seconds of strong laterals, max positive, negative and lateral g, max speed, ride length and duration, number of inversions, drops and airtime moments, and similar. Use the heart-line force measurement from TASK-95.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Scores on an approximately 0-10 scale with calibration notes
- [ ] #2 Report lists the airtime, force and speed statistics
- [ ] #3 Covered by tests on known circuits
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Owner request 2026-11-04: inform scores by ASTM F2291 and reasoned judgement of why people like coasters (airtime most important; laterals and positives less; element variety with diminishing returns for repeated inversions, little for airtime; pacing; whip and aggression; theming; ride time; max and average speed). The running job delivers the statistics and the 0-10 rescale; a Fable plan (docs/superpowers/plans/2026-11-04-coaster-scoring.md) designs the scoring model to implement afterwards.

2026-11-05: branch ride-stats (6b5ca09) is complete apart from re-recording the Velocity Viper scoring-panel screenshot (it grew with the self-explanatory contributions; the re-record was queued behind other browser runs). Lint, full feature project and 8/9 journeys passed; merge after the re-record, then close TASK-104 and TASK-113. The panel is too wordy: TASK-130 makes it concise.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Explainable coaster scoring is on master: per-car heartline features, model force bands matching the overlay's FORCE_RATING_BANDS, an ASTM F2291 comfort envelope as a soft bound with report notes, contributions per score that name the force axis, value, limit and duration, track theming, guests reading the real ratings (soft nausea tolerance), and whole-train statistics (airtime, forces, speed and layout) in the testing panel. Calibrated to the owner's enthusiast ordering: Intamin Blitz 11.1, RMC 10.8, B&M hyper 6.6, accelerator 5.8, Arrow looper 4.1, family wooden 3.0; excitement floored at 0. The game's own circuits score honestly low until rebuilt (TASK-101 task 7); the panel's wording is made concise in TASK-130. Gates on merged master: sim lib, circuit and statistics tests, wasm 143, report types, full feature 187/187, journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
