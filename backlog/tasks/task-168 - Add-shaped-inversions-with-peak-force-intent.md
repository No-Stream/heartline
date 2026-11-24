---
id: TASK-168
title: Add shaped inversions with peak-force intent
status: Done
assignee: []
created_date: '2026-11-22 04:58'
updated_date: '2026-11-24 04:57'
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
- [ ] #1 Shaped vertical loops, zero-g rolls, and Raven turns solve for entry speed, train length, and peak vertical g (3.5-4.5 g) via ShapedProfile
- [x] #2 Steel-structure shaped vertical loops work on wooden_twister; zero-g rolls and Raven turns work on velocity_viper and hybrid; non-inverting models return UnsupportedScope
- [ ] #3 Vertical loops and Raven turns support Floaty/Neutral/PressedIn top feel; height scales monotonically with entry speed; peak roll rate <= 200 deg/s
- [ ] #4 Wire types, builder palette/inspector, design-tools CLI, and ride/save round-trips support all three shaped inversions
- [ ] #5 Native vs WASM parity, acceptance matrices under both physics configs, and Playwright E2E tests pass with classic digests unchanged
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed on master dff412d (CI green). Raven top feel aims the train-average crest g at the target within 0.35 g; per-car spread bounded only by the global force limits (orchestrator decision, consistent with targets-vs-global-envelope). Shared profile heart-line noise fixed (worst sample-to-sample tangential jump 4.7 g to 0.1 g). Saves are version 10; version 9 and older saves keep their original shaped-hill geometry (HillProfileNodes marker). 12 low-speed raven feel combinations per hand are refused with TopFeelOutOfReach.
<!-- SECTION:NOTES:END -->
