---
id: TASK-126
title: Evaluate rider forces with ASTM-style filtering and duration limits
status: To Do
assignee: []
created_date: '2026-11-05 16:09'
updated_date: '2026-11-07 01:33'
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

2026-11-05 evening: third attempt stopped (HalfCorkscrewLeftDown had no passing window anywhere in 0.25-100 m/s, unexplained; 7 inversion and 6 rated tests still failing; scoring still uses a separate ASTM envelope; live preview p95 142 ms on Timber Tumbler). Branch force-smoothing is checkpointed at b240068 with master merged (not mergeable). Paused for a proper plan: Fable is writing docs/superpowers/plans/2026-11-05-force-smoothing.md.

2026-11-05: plan committed (bc3a68e). Implementing on branch force-rule; task 1 dispatched.
2026-11-06 task 4 (branch force-rule, commits 5c69f0d and 3cf31de): rated windows now come from crates/sim/src/track/rated_windows.generated.rs (make generate-rated-windows). Rule: maximum = highest speed where every allowed train length stays within the comfortable limits on every axis after filtering (nothing above Great); minimum rises only for clearance or tail; zero-g rolls, stalls and half corkscrews keep the 200 deg/s roll-rate rule; heartline rolls, banked straights and descending half corkscrews are design caps. One minimum for every train length (effective_rated_speeds removed). No ejector hill needed relabelling. 7 maxima fell, 13 minima rose, most maxima rose. Details: docs/reviews/task-126-rated-windows.md. Open owner calls: cap the rollouts (now up to 44.7 m/s), raise any design cap; LoopRight keeps 24.73 m/s minimum while Loop rose to 24.87 (mirror asymmetry unexplained).

2026-11-06 afternoon: merged master (7f4502d) into force-rule (a72f1cb). Table now covers 101 templates: speed-matched turns keep their design windows (design caps); floater and strong floater hills stop where a default train would get more than Good airtime (FloaterHill12x80 25.25 m/s, WoodenEjectorHill8x64 22.85 m/s). Rebuilt Timber Tumbler and Velocity Viper and Briar Blitz enter every rated piece inside its window (make circuit-report). Master's coarse-step preview force correction dropped (branch previews at 5 ms, filtered). Three screenshot baselines re-recorded (f47f114, bb933ce).
2026-11-06: branch force-rule has the generated window table for 101 pieces with master 7f4502d merged; Rust and unit gates pass; a full browser run was still going when the machine was turned off (tmux fr-full). Still to do: merge current master (fast turns, Daisy Dash, Horizon Hawk add pieces; regenerate with make generate-rated-windows), full gates, merge, close-out (plan task 6).
<!-- SECTION:NOTES:END -->
