---
id: TASK-163
title: Finish search places shaped elements
status: To Do
assignee: []
created_date: '2026-11-21 06:31'
labels: []
dependencies: []
ordinal: 159000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The finish-the-circuit search (TASK-159, crates/sim/src/track/construction/completion.rs) only places classic pieces and brakes. On Horizon Hawk with four or more pieces removed it reports 'too fast' because classic turns exceed lateral limits at 22 m/s, while shaped turns solved for the entry speed would fit. Let the search place shaped hills and turns for models that support them, solved for the actual entry speed, within its existing time budget, and keep classic-only results unchanged for other models.
<!-- SECTION:DESCRIPTION:END -->
