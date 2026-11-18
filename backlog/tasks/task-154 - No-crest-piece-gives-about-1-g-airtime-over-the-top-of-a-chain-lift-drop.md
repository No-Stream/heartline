---
id: TASK-154
title: No crest piece gives about -1 g airtime over the top of a chain-lift drop
status: To Do
assignee: []
created_date: '2026-11-18 17:36'
labels:
  - coaster
  - track
dependencies: []
priority: low
ordinal: 151000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while building the new demo (TASK-140). The owner wants about -1 g airtime at the crest of the main drop. The chain releases the train at 5 m/s, so crest airtime comes from the back car being pulled over by the falling train, and it is set by the crest transition's curvature. Measured on the Timber Tumbler (6 cars) after a 60 m chain lift, worst car: TerrainFall then LevelToGentleDown and GentleToSteepDown -0.24 g; ShortFall pre-drop -0.25 g; LevelToSixtyDown -0.43 g; WideLevelToSixtyDown -0.60 g. The front car stays above +0.2 g in all of them. None reaches -1 g. Real wooden and hybrid first drops get this from a tighter parabolic crest. Proposal: a crest piece (for example LevelToSteepDown with a parabolic profile designed for -1 g on the back car at 5 m/s chain speed), checked with the native report.
<!-- SECTION:DESCRIPTION:END -->
