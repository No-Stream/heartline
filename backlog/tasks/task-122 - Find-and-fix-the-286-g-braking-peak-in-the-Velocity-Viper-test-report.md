---
id: TASK-122
title: Find and fix the 286 g braking peak in the Velocity Viper test report
status: Done
assignee: []
created_date: '2026-11-05 04:25'
updated_date: '2026-11-05 06:00'
labels:
  - physics
dependencies: []
priority: high
ordinal: 122000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The ride statistics job (TASK-104 branch) saw a raw 285.9 g braking peak in the live Velocity Viper test report. Scores and statistics read peak forces, so a spike like this corrupts them. Same class as the lift-entry speed snap fixed earlier (2.5 g longitudinal bound test).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The 286 g braking peak came from the lift-entry speed snap that master had already fixed (352e2ca); the ride-stats branch predated the fix. Added a raw per-step longitudinal bound (2.5 g) over full construction-style rides for Velocity Viper, Briar Blitz and the demo circuit; it fails with the old snap (-95 g). Timber Tumbler's old Airtime Hill gives a real 8 g longitudinal at the heart at 43 m/s (abrupt curvature onset), so it is excluded from the raw check until TASK-101 replaces the hill.
<!-- SECTION:FINAL_SUMMARY:END -->
