---
id: TASK-48
title: Fix food facilities refusing placement with missing access
status: Done
assignee: []
created_date: '2026-11-03 16:40'
updated_date: '2026-11-03 18:01'
labels:
  - world
  - bug
milestone: m-1
dependencies: []
priority: high
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Owner report: placing food facilities says 'missing access'. Likely a regression from explicit path edge connections: the facility entrance check may now require an explicit edge that a neighbouring path does not have.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Food, drink and other facilities place next to a path in every direction and connect to it
- [x] #2 A browser test places each facility kind through the UI next to an existing path
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Facility placement now derives facing from a flat same-height path beside the footprint and shows that facing in the panel preview. The browser test places every facility kind through the UI, checks food stalls on all four sides, and confirms a guest visits the connected stall. Targeted placement flow passes; the existing ride screenshot assertions currently show repeatable 0.01 pixel-difference ratios.

Follow-up check: the screenshot drift is caused by concurrent renderer edits in web/src/facilities.ts and web/src/render/scene.ts that add heightDepth handling for facility sprite depth. The placement browser flow passes; screenshot baselines were left unchanged.

Access direction now comes from the Rust simulation and is exposed to the worker through WASM for both placement and hover preview. Rust tests cover preferred-facing order and both ride access tiles. In the current targeted browser run, facility placement and the rotation 0 screenshot passed; the rotation 2 screenshot still differs by 936 pixels (0.01 ratio), so that assertion remains unresolved.
<!-- SECTION:NOTES:END -->
