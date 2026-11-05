---
id: TASK-123
title: 'Preselect speed-matched sizes for loops, rolls and other rated elements'
status: Done
assignee: []
created_date: '2026-11-05 05:43'
updated_date: '2026-11-05 08:09'
labels:
  - construction
  - coaster
dependencies:
  - TASK-105
priority: high
ordinal: 123000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner request 2026-11-04 (force-vector-design style building): when a rated element (loop, half loop, corkscrew, heartline roll, zero-g roll, stall) is chosen, preselect the size or variant whose rated window best contains the predicted entry speed from the construction estimate (TASK-105), and show its expected peak forces with weak/good/great/too much bands before placing. If no variant fits, say so. Hills get the same behaviour in TASK-101.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Selecting a rated element with several sizes recommends the one with the greatest margin inside its rated speed window at the estimated entry speed (nearest window when none fits); explicit player choices are marked exact and never redirected. The readout previews the candidate's peak forces with rating bands and flags stalls or partial estimates. Adds 6-10 ms per selection or placement on the player circuits. Gates: wasm tests, build, lint, report types, npm test 364, construction-estimate and construction-controls specs, make journeys 9/9.
<!-- SECTION:FINAL_SUMMARY:END -->
