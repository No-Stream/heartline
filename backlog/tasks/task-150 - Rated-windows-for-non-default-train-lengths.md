---
id: TASK-150
title: Rated windows for non-default train lengths
status: To Do
assignee: []
created_date: '2026-11-18 17:35'
labels:
  - physics
  - builder
dependencies: []
priority: medium
ordinal: 149000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the TASK-126 close-out (2026-11-18): rated speed windows are generated for each model's default train, but players can pick any length in cars_per_train. Tested at other lengths, short Briar Blitz trains reach only 3.9-4.8 m/s at the SteelLoop minimum (test needs 5 m/s), and an eight-car Timber Tumbler train fails connected-track clearance at the LevelToSixtyUp and HalfCorkscrewLeftDown minima. Decide whether windows should be generated per train length, or the builder readout should warn when the chosen train differs from the default, then implement it.
<!-- SECTION:DESCRIPTION:END -->
