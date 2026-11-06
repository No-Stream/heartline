---
id: TASK-131
title: Chain lifts keep pulling until the last car leaves the lift
status: Done
assignee: []
created_date: '2026-11-06 00:11'
updated_date: '2026-11-06 19:42'
labels: []
dependencies: []
priority: high
ordinal: 131000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-126 measurements: on the Timber Tumbler player circuit the lift drive stops when the front car leaves the last lift piece while rear cars are still on the incline; the train crests at about 3 m/s, stalls at 205 m and rolls back (both test run and preview). A real chain or cable lift keeps pulling the train until its last car is off the lift. Make lift drive (chain speed hold) apply while any car is on a lift piece, keep the coast-onto-lift behaviour, and check that the change in forces on the player circuits and journeys is the intended one.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Chain lifts keep the train at lift speed while any car is on a lift piece (launches and holds keep priority; trains still coast onto the chain). This made Timber Tumbler's old circuit reach its 43 m/s Airtime Hill, so it merged together with the force-designed Timber rebuild (good-circuits). The block-capacity interlock fallback became unreachable and was removed (owner: no fallbacks). On master as 6b75450; gates: sim, wasm 145, feature 189, journeys 9.
<!-- SECTION:FINAL_SUMMARY:END -->
