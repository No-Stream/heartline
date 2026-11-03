---
id: TASK-79
title: >-
  Mark chain lift pieces in the placed-track strip and drop internal words from
  piece names
status: To Do
assignee: []
created_date: '2026-11-03 23:11'
labels:
  - coaster
  - interface
dependencies: []
priority: medium
ordinal: 79000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the TASK-70 palette pass (b4c2e8d): the placed-track strip in the builder does not show which pieces carry a chain, because the construction state from sim-wasm and worker.ts has no per-piece kind. Add the kind to the construction state and mark lift pieces in the strip. Also piece names still show internal words, e.g. 'Climbing standalone small left turn'; give players plain names (e.g. 'Climbing small left turn') without breaking the journeys' locators.
<!-- SECTION:DESCRIPTION:END -->
