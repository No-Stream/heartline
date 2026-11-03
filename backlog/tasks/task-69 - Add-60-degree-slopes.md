---
id: TASK-69
title: Add 60-degree slopes
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 22:55'
labels:
  - coaster
dependencies: []
priority: high
ordinal: 69000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 7. UpSixty/DownSixty and six transition pieces, model maximum slopes, client labels.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented UpSixty/DownSixty catalog geometry and six 60-degree slope pieces. The constant piece uses an 8 m horizontal run and 14 m rise (60.255 degrees); transition peaks at 1.9614 g at 25 m/s. Timber Tumbler and Velocity Viper allow the new pieces with UpSixty lift limits; Briar Blitz remains capped at UpSteep. Enabled the connector maximum-slope capability test. The Timber Tumbler journey now passes through the 60-degree lift, crest, 60-degree drop, banked turns, and airtime hill, but still does not return to the station or close the circuit. Rust sim (391 tests), sim-wasm (92), Vitest (211), TypeScript typecheck, formatting, build, piece catalog, construction, and Timber Tumbler journey checks passed.
<!-- SECTION:NOTES:END -->
