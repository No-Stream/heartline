---
id: TASK-128
title: Design every solved element with heart-point speed
status: To Do
assignee: []
created_date: '2026-11-05 16:38'
updated_date: '2026-11-05 19:44'
labels:
  - physics
dependencies: []
priority: medium
ordinal: 128000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the TASK-125 crest solver design (2026-11-05, docs/superpowers/plans/2026-11-05-top-hats.md, 'Back-car crest solver'): the profile solver computes design force with rail speed, but the heart moves at v * R_h / R_r, so measured heart force is kappa_h v^2 (R_h/R_r)^2 / g. Tight crests and loops therefore give 25-35 % more airtime or force than designed. Rated windows are unaffected (they come from full-train simulations of the actual geometry), but design targets and feel are off. TASK-125 fixes it only for the new Blitz top hat behind a heart_speed_forces flag. Migrating the rest changes the geometry of every loop, hill and crest and affects saved parks and replays (one-way door): decide with the owner, then regenerate tables, re-rate windows, rehash sessions and migrate saves.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-05 owner: the heart-speed fix is fine to dispatch, not urgent.
<!-- SECTION:NOTES:END -->
