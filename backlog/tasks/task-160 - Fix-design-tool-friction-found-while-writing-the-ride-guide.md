---
id: TASK-160
title: Fix design tool friction found while writing the ride guide
status: To Do
assignee: []
created_date: '2026-11-21 03:05'
labels: []
dependencies: []
ordinal: 156000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while writing docs/designing-rides.md (2026-11-20): (1) the state line's 'can finish' reads 'Checking for a connection to the station.' even where finish succeeds, and 'can close' stays false after finish although close works; (2) check labels the sustainedMaxVerticalG result 'maximum vertical g' though it checks the sustained value; (3) rated-window and force-headroom failures only make check exit 1 with designMargins: true, and a failing check prints its report on stderr; (4) text output rounds shaped hill values to one decimal (-0.85 g prints -0.8), JSON is exact; (5) help omits new --cars; (6) estimate prints no forces for shaped elements, only probe does; (7) no command writes the expect block. Some overlap with the circuit-finish branch (TASK-159), which changes finish; fix after it merges.
<!-- SECTION:DESCRIPTION:END -->
