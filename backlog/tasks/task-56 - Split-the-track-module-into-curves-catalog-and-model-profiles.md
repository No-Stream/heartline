---
id: TASK-56
title: 'Split the track module into curves, catalog and model profiles'
status: Done
assignee: []
created_date: '2026-11-03 16:47'
updated_date: '2026-11-04 00:17'
labels:
  - refactor
  - coaster
milestone: m-4
dependencies: []
documentation:
  - docs/reviews/2026-11-03-track-physics-code.md
priority: medium
ordinal: 56000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Track review finding 1: crates/sim/src/track/mod.rs is about 1,800 lines mixing curve maths, the piece catalog with mirrored generation, and model profiles. Rating calibration values are inline (finding 8).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Curves, catalog and models live in separate modules with existing exports kept; rating parameters are named; the unused SupportError::SearchExhausted is resolved
- [x] #2 Behaviour is unchanged unless stated, and the relevant tests pass while editing, with make ci green at the end
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Closed 2026-11-03: make ci at 9987c3e passed everything except the piece-catalog baselines (expected after the lift piece removal), refreshed in 8be816e.
<!-- SECTION:NOTES:END -->
