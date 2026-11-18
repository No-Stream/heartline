---
id: TASK-151
title: Hill readout predicts much weaker airtime than the train gets
status: To Do
assignee: []
created_date: '2026-11-18 17:36'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 148000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while building the new demo (TASK-140). The hill readout (predicted_hill_forces, crates/sim/src/track/hill_forces.rs, wired in sim-wasm construction.rs construction_choices_with_preview) models a point mass at the hill's entry speed. A real train's front and back cars cross the crest faster than a point mass because the rest of the train is still on the slopes, so they get stronger airtime; the point mass behaves like a middle car. On the demo, hill readout versus placed estimate and test run (front car): Ejector hill 16x96 at 25.5 m/s -0.41 g vs -0.82 g; Stall at 25.1 m/s -0.26 vs -0.41; Zero-g roll at 24.2 m/s -0.27 vs -0.37; Ejector hill 8x64 at 23.6 m/s -0.70 vs -1.08. Positive peaks are overstated by 0.1-0.25 g. The front-car minimum on the 16x96 is 41 m into the 103 m piece, at 21.5 m/s, while a point mass crests at 18.4 m/s. Steps: build the default demo layout in a new park, select Ejector hill 16x96 after the main drop, compare the hill readout with the force estimate after placing. Fix direction: run the readout through the same train preview as the candidate force estimate (construction_candidate_force_preview_json) and report the worst car, or at least front and back car.
<!-- SECTION:DESCRIPTION:END -->
