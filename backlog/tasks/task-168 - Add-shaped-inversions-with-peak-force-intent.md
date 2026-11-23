---
id: TASK-168
title: Add shaped inversions with peak-force intent
status: In Progress
assignee: []
created_date: '2026-11-22 04:58'
updated_date: '2026-11-23 05:24'
labels:
  - shaped-elements
dependencies: []
priority: medium
ordinal: 162000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up from TASK-156 / TASK-167: add named shaped inversions (vertical loops, zero-g rolls, Raven turns, etc.) sized for entry speed and peak-force intent on top of the per-model ShapedProfile system.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Shaped vertical loops, zero-g rolls, and Raven turns solve for entry speed, train length, and peak vertical g (3.5-4.5 g) via ShapedProfile
- [x] #2 Steel-structure shaped vertical loops work on wooden_twister; zero-g rolls and Raven turns work on velocity_viper and hybrid; non-inverting models return UnsupportedScope
- [x] #3 Vertical loops and Raven turns support Floaty/Neutral/PressedIn top feel; height scales monotonically with entry speed; peak roll rate <= 200 deg/s
- [x] #4 Wire types, builder palette/inspector, design-tools CLI, and ride/save round-trips support all three shaped inversions
- [x] #5 Native vs WASM parity, acceptance matrices under both physics configs, and Playwright E2E tests pass with classic digests unchanged
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Initial reassessment at 09740d4: the branch contains broad solver, UI, design-tool, save-test, and test changes, but its prior completion claims require independent verification. Master has since added save versions 7–8, multi-coaster state, a new PieceId, and declared-piece inversion counting; integration and final gates remain outstanding. Baseline build and test assessment is in progress.

Assessment: solver, wire, UI, design-tool and save changes are present. The visible acceptance matrix covers loop models and defaults broadly, but zero-g-roll peak steps, Raven peak/top-feel and hand combinations, speed perturbations, and a Hybrid browser route are not evidently covered to the full extent described by the plan. Build and test results remain pending.

Correction: assert_inversion_envelope does test ±0.5 m/s under both physics configurations. The remaining visible matrix gaps are the zero-g-roll and Raven force/top-feel combinations and Hybrid browser coverage; save tests also need placed-piece coverage against current master versions.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented shaped vertical loops, zero-g rolls, and Raven turns with peak-force intent (3.5-4.5 g) and loop/Raven top feel (Floaty/Neutral/PressedIn) across velocity_viper, hybrid, and steel-loop wooden_twister on top of ShapedProfile, including solver, wire/TS types, builder palette/inspector/readouts, design-tools CLI, v6 save round-trips, 17-case native-vs-WASM parity, and full Rust/Vitest/Playwright E2E/ride-agreement verification.
<!-- SECTION:FINAL_SUMMARY:END -->
