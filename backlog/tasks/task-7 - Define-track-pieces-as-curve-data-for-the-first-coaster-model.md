---
id: TASK-7
title: Define track pieces as curve data for the first coaster model
status: Done
assignee: []
created_date: '2026-11-03 01:52'
updated_date: '2026-11-03 02:39'
labels:
  - coaster
  - simulation
milestone: m-1
dependencies:
  - TASK-2
documentation:
  - docs/superpowers/specs/2026-11-02-theme-park-sim-design.md
priority: high
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every piece is a continuous 3D curve plus a roll profile with entry and exit states on a grid. Model limits are data. Start with the steel-track hybrid model and its playful manufacturer name, such as Rocky Road Construction. Design section 3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Piece format covers turns, banks, slopes, half-tile offsets and inversions
- [x] #2 Model definition lists allowed pieces, radii, inversions, lifts, supports and trains, checked against real rides
- [x] #3 An automated test confirms each piece's entry and exit connect exactly to the curve ends
- [x] #4 No real manufacturer or ride names appear in player-facing data
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Quintic Hermite centre lines with arc-length tables, 8 headings, half-tile offsets, inversions, lift, brakes and station. First model is Timber Tumbler by Rocky Road Construction. Pieces end with zero curvature, so back-to-back turns dip to straight at each join; revisit if riders feel it in physics.
<!-- SECTION:FINAL_SUMMARY:END -->
