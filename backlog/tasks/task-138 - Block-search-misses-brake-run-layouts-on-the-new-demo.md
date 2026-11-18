---
id: TASK-138
title: Block search misses brake-run layouts on the new demo
status: To Do
assignee: []
created_date: '2026-11-06 21:31'
labels: []
dependencies: []
priority: medium
ordinal: 138000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while rebuilding the demo coaster (branch demo-circuit): automatic block placement finds only one train for the new demo even after 60 million work steps, although a valid two-train layout exists (a stopping run ending at the lift crest, the mid-course brake run, and the final brakes). BlockPlacementSearch::new in crates/sim/src/track/blocks/mod.rs spreads its starting guesses evenly and never tries existing Brakes pieces plus the lift-crest run. Seed the search with runs on existing brakes and the lift crest, and test it on the new demo (two trains).
<!-- SECTION:DESCRIPTION:END -->
