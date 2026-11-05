---
id: TASK-126
title: Evaluate rider forces with ASTM-style filtering and duration limits
status: To Do
assignee: []
created_date: '2026-11-05 16:09'
updated_date: '2026-11-05 22:27'
labels:
  - physics
  - research
dependencies: []
priority: medium
ordinal: 126000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Research for TASK-125 (2026-11-05) suggests ASTM F2291 evaluates vertical acceleration after low-pass filtering (about 5 Hz) and treats events under about 200 ms separately (clause 7.1.4.2 per ASTM work-item pages), with brief negative spikes allowed further than sustained -1.5 g (commonly cited about -2.0 g for very short durations; exact curves need the paid standard). The game checks raw per-step forces. Decide with the owner whether to adopt filtered, duration-aware limits for rated windows, scoring and the building overlay; it would change many rated windows, so measure the effect first. Buy or consult F2291-26 Section 7 for exact limits.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-05 owner: yes, smooth forces. Example: a 4.5 g moment in a loop is fine if brief, as on a Schwarzkopf. Use filtered, duration-aware limits for rated windows, scoring and the building overlay.

2026-11-05 checkpoint 7d54d73 on branch force-smoothing (NOT mergeable): shared four-pole 5 Hz filter and duration-aware evaluator (sustained limit, brief cap +0.5 g for at most 0.2 s), scoring and overlay wired, save version bumped. Open: (1) the window scan's entry-speed solver asserts when a requested speed is unreachable after a preceding rise, so SixtyToSteepDown and both descending half corkscrews showed no passing samples (not proven infeasible); fix it and rerun the full 94-template scan with design caps (pieces whose forces never bind keep their current maximum); (2) 7 inversion-physics and 6 rated-element tests fail; wasm block_analysis.rs:215 panics on an open coaster; 2 clippy findings; 5 feature failures; (3) the accurate 5 ms construction preview is 10x slower (Timber Tumbler 13 -> 154 ms, Briar Blitz 31 -> 337 ms): needs a coarser preview step for the live overlay or cancellable slicing; (4) master now uses postcard save version 2 (TASK-118), so this branch's version bump must become 3. Then verify the hill airtime guard (no 'dangerous' at any new maximum).
<!-- SECTION:NOTES:END -->
