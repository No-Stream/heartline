---
id: TASK-70
title: Show the chain toggle and geometry-based groups in the builder palette
status: To Do
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 23:11'
labels:
  - coaster
  - interface
dependencies: []
priority: high
ordinal: 70000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 8. Chain toggle (key H), construction-set-chain action, groups by geometry, Climbing/Descending names; owner playtest.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Commit b4c2e8d. Added a visible Chain lift toggle (aria-pressed, aria-keyshortcuts H, key hint, On/Off state, hint line) between the piece steppers and the palette; it dispatches construction-set-chain with the opposite of the worker's chain state, H still goes through chain-toggle in main.ts (no main.ts or worker.ts change). Refused pieces under chain mode use the existing hatched tile plus tooltip and status reason. Geometry grouping and Climbing/Descending names were already in place; changed names so Begin/End turns read 'Begin climbing small left turn' (Standalone and plain forms unchanged), and slope stepper values now carry angles (Gentle up 27°, Steep up 45°, Very steep up 60°). Tests: Vitest for names and labels; new web/e2e/chain-toggle.spec.ts (mouse and H toggling checked against chain flags posted by the worker, refused descent reason, curved chain lift placed; passed 9 of 9 repeats); fixture test in construction-controls.spec.ts. Updated construction-preview baseline for the toggle row and slope label. Not done: placed-track chips do not mark lift pieces (would need a kind field in construction_track rows and worker.ts). Owner playtest still open.
<!-- SECTION:NOTES:END -->
