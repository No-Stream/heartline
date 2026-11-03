---
id: TASK-68
title: Add climbing and descending turns
status: Done
assignee: []
created_date: '2026-11-03 20:05'
updated_date: '2026-11-03 22:24'
labels:
  - coaster
dependencies: []
priority: high
ordinal: 68000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan: docs/superpowers/plans/2026-11-03-flexible-track-and-gameplay-tests.md, work item 6. Generated slope versions of every turn, lookup by id and entry, sloped_turn_slopes style field.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Generated gentle climbing and descending turn variants for all three models with connector-aware selection, curved lift coverage, geometry-based palette grouping, and slope-derived names. Large turns use 12 grid units of rise; the 15 m/s midpoint force deviation is 0.0081 g versus 0.0209 g for 13. Release choices() on the 150-piece test measured 30.89 ms before and 31.59 ms after (1.02x). Simulation and WASM tests, npm test, typecheck, formatting, build, and requested feature browser specs passed. Timber Tumbler reaches the expected TASK-69 missing 60-degree slope assertion. Catalog and construction preview snapshots were visually reviewed and refreshed.
<!-- SECTION:NOTES:END -->
