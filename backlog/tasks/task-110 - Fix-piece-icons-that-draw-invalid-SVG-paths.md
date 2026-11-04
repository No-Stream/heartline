---
id: TASK-110
title: Fix piece icons that draw invalid SVG paths
status: Done
assignee: []
created_date: '2026-11-04 17:59'
updated_date: '2026-11-04 18:37'
labels: []
dependencies: []
priority: medium
ordinal: 110000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found 2026-11-04 during the builder panel reorganisation: the browser console shows 'MNaN NaN' SVG path errors from the piece icons in the builder palette. Present before the panel change. Probably some piece shapes (likely newer ones: drops, 60-degree transitions, inversions or short offsets) produce NaN in the icon projection. Find which, fix the icon path generation, and add a test that every catalog piece icon has a finite path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 No NaN in any piece icon path
- [x] #2 A test checks every catalog piece icon
<!-- AC:END -->
