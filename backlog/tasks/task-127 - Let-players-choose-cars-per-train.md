---
id: TASK-127
title: Let players choose cars per train
status: Done
assignee: []
created_date: '2026-11-05 16:24'
updated_date: '2026-11-05 19:25'
labels:
  - coaster
  - ui
dependencies: []
priority: high
ordinal: 127000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-125 (2026-11-05): the game has no control for choosing how many cars a coaster's trains have; trains use the model's default length. Short-train elements such as a Blitz-style top hat need it, and train length changes capacity, forces and scores. Add a cars-per-train control in the builder's Operate tab within each model's allowed range, show capacity and how forces change, re-run block analysis when it changes, and keep it in saves and session replays.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The builder's Operate tab has a cars-per-train control within per-model ranges (Timber Tumbler 4-8, Velocity Viper 3-6, Briar Blitz 4-12; defaults unchanged). Train length, capacity, block analysis, the construction estimate, test runs and train-dependent rated-speed readouts follow it; saves keep it and session recordings include the new command. Gates on merged master: wasm 138, sim lib 570, npm 392, full feature 185/185, journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
